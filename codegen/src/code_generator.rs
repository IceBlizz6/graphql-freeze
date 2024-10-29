use std::fs;
use std::process;
use futures::future;
use std::io::{BufRead, Write};
use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use crate::schema::{Enum, GqlDocument, GqlType, Object};
use crate::code_writer::CodeFile;
use crate::code_writer::CodeFileOptions;

const EMBEDDED_HASH_PREFIX: &'static str = "// hash:";

fn io_error_abort(context: &str, error: std::io::Error) -> ! {
    eprintln!("{}", context);
    eprintln!("IO error: {}", error.to_string());
    process::exit(1)
}

pub async fn write_files(
    document: GqlDocument,
    output_directory: PathBuf,
    options: CodeFileOptions,
    runtime: &str
) {
    if !output_directory.exists() {
        match fs::create_dir(&output_directory) {
            Ok(()) => (),
            Err(error) => io_error_abort(
                &format!("Unable to create output directory {}, does the parent folder exist?", output_directory.display()),
                error
            )
        }
    }

    let create_index_task = async {
        let path = &output_directory.join("index.ts");
        if path.exists() {
            println!("index.ts - already exists");
        } else {
            write_index_ts(&path, &options, runtime);
            println!("index.ts - created");
        }
    };

    let write_schema_task = async {
        let content = write_schema_ts(
            &document.inputs,
            &document.outputs,
            &document.scalars,
            &document.enums,
            &options,
            &runtime
        );
        let path = &output_directory.join("schema.ts");
        let result = overwrite_on_diff(path, &content, &options);
        result.log("schema.ts");
    };

    let write_codec_task = async {
        let content = write_codec_ts(&document.inputs, &document.outputs, &options, runtime);
        let path = &output_directory.join("codec.ts");
        let result = overwrite_on_diff(path, &content, &options);
        result.log("codec.ts");
    };

    future::join3(create_index_task, write_schema_task, write_codec_task).await;
}

fn write_index_ts(
    file_path: &PathBuf,
    options: &CodeFileOptions,
    runtime: &str
) {
    let template = include_str!("../resources/client.template")
        .replace("__RUNTIME_PACKAGE__", runtime)
        .replace("\t", &options.indent)
        .replace("\n", &options.line_break);
    let mut file = match File::create_new(file_path) {
        Ok(file) => file,
        Err(error) => io_error_abort(
            &format!("Unable to create new file {}", file_path.display()),
            error
        )
    };
    match file.write_all(&template.as_bytes()) {
        Ok(()) => (),
        Err(error) => io_error_abort(
            &format!("Unable to write to new file {}", file_path.display()),
            error
        )
    }
}

fn overwrite_on_diff(file_path: &PathBuf, new_content: &str, options: &CodeFileOptions) -> FileWriteResult {
    let new_content_hash = crc32fast::hash(new_content.as_bytes());

    if file_path.exists() {
        let skip = if let Some(hash) = read_embedded_hash(file_path) {
            hash == new_content_hash
        } else {
            false
        };

        if skip {
            FileWriteResult::NoChange
        } else {
            let mut file = match File::create(file_path) {
                Ok(file) => file,
                Err(error) => io_error_abort(
                    &format!("Unable to create or truncate file {}", file_path.display()),
                    error
                )
            };
            match write_all_with_hash(&mut file, new_content, new_content_hash, &options) {
                Ok(()) => (),
                Err(error) => io_error_abort(
                    &format!("Unable to write to file {}", file_path.display()),
                    error
                )
            }
            FileWriteResult::Overwritten
        }
    } else {
        let mut file = match File::create_new(file_path) {
            Ok(file) => file,
            Err(error) => io_error_abort(
                &format!("Unable to create file {}", file_path.display()),
                error
            )
        };
        match write_all_with_hash(&mut file, new_content, new_content_hash, &options) {
            Ok(()) => (),
            Err(error) => io_error_abort(
                &format!("Unable to write to file {}", file_path.display()),
                error
            )
        }
        FileWriteResult::Created
    }
}

fn read_embedded_hash(path: &PathBuf) -> Option<u32> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => io_error_abort(
            &format!("Failed while trying to open {} in order to read hash", path.display()),
            error
        )
    };
    let mut reader = BufReader::new(&file);
    let mut hash_line = String::new();
    match reader.read_line(&mut hash_line) {
        Ok(_) => (),
        Err(error) => io_error_abort(
            &format!("Failed while trying to read first line from {}", path.display()),
            error
        )
    }

    if hash_line.starts_with(EMBEDDED_HASH_PREFIX) {
        let offset = EMBEDDED_HASH_PREFIX.chars().count();
        match hash_line.get(offset..) {
            Some(hash_string) => {
                let file_hash = hash_string.trim_end();
                match file_hash.parse() {
                    Ok(hash) => Some(hash),
                    Err(_) => None
                }
            }
            None => None
        }
    } else {
        None
    }
}

fn write_all_with_hash(file: &mut File, new_content: &str, hash: u32, options: &CodeFileOptions) -> Result<(), std::io::Error> {
    let content_with_hash = format!("{}{}{}{}", EMBEDDED_HASH_PREFIX, hash, options.line_break, new_content);
    file.write_all(content_with_hash.as_bytes())
}

enum FileWriteResult {
    Overwritten,
    NoChange,
    Created
}

impl FileWriteResult {
    fn log(&self, file_name: &str) {
        match &self {
            FileWriteResult::Created => println!("{} - created", file_name),
            FileWriteResult::NoChange => println!("{} - skipped (no change)", file_name),
            FileWriteResult::Overwritten => println!("{} - overwritten", file_name)
        }
    }
}

fn write_schema_ts(
    inputs: &Vec<Object>,
    outputs: &Vec<Object>,
    scalars: &Vec<String>,
    enums: &Vec<Enum>,
    options: &CodeFileOptions,
    runtime: &str
) -> String {
    let mut file = CodeFile::new(options);
    file.line(&format!("import {{ Scalar }} from \"{}\"", runtime));
    file.line(&format!("import {{ QFun, QList, QNull, QObject, QScalar, QEnum }} from \"{}\"", runtime));
    file.blank_line();

    file.begin_indent("export interface Scalars {");
    for scalar in scalars {
        file.line(&format!("{}: {}", scalar, "Scalar<unknown, unknown>"));
    }
    file.end_indent("}");

    file.blank_line();
    file.begin_indent("export function createScalars<T extends Scalars>(scalars: T): T {");
    file.line("return scalars");
    file.end_indent("}");

    for enum_def in enums {
        file.blank_line();
        file.begin_indent(&format!("export enum {} {{", enum_def.name));
        for member in &enum_def.values {
            file.line(&format!("{} = \"{}\",", member, member));
        }
        file.end_indent("}");
    }

    if !outputs.is_empty() {
        file.begin_indent("export type ObjectSchema = {");
        for output in outputs {
            file.begin_indent(&format!("{}: {{", output.name));
            for field in &output.fields {
                file.line(&format!("{}: {}", field.name, gql_type_to_code(&field.field_type)));
            }
            file.end_indent("}");
        }
        file.end_indent("}");
        file.blank_line();
    }

    if !inputs.is_empty() {
        file.begin_indent("export type InputObjectSchema = {");
        for input in inputs {
            file.begin_indent(&format!("{}: {{", input.name));
            for field in &input.fields {
                file.line(&format!("{}: {}", field.name, gql_type_to_code(&field.field_type)));
            }
            file.end_indent("}");
        }
        file.end_indent("}");
    }
    file.to_string()
}

fn gql_type_to_code(gql_type: &GqlType) -> String {
    match gql_type {
        GqlType::List(inner) => format!("QList<{}>", gql_type_to_code(inner)),
        GqlType::Nullable(inner) => format!("QNull<{}>", gql_type_to_code(inner)),
        GqlType::Scalar(name) => format!("QScalar<\"{}\">", name),
        GqlType::Enum(name) => format!("QEnum<{}>", name),
        GqlType::Object(name) => format!("QObject<\"{}\">", name),
        GqlType::Function { inputs, output } => {
            let input_as_code: Vec<String> = inputs.iter().map(|arg| {
                let name = arg.name.clone();
                let gql_type = &arg.argument_type;
                if let GqlType::Nullable(_) = gql_type {
                    format!("{}?: {}", name, gql_type_to_code(&gql_type))
                } else {
                    format!("{}: {}", name, gql_type_to_code(&gql_type))
                }
            }).collect();
            format!("QFun<{{ {} }}, {}>", input_as_code.join(", "), gql_type_to_code(output))
        }
    }
}

fn write_codec_ts(
    inputs: &Vec<Object>,
    outputs: &Vec<Object>,
    options: &CodeFileOptions,
    runtime: &str
) -> String {
    let mut file = CodeFile::new(options);
    file.line(&format!("import {{ Scalars }} from \"{}\"", "./index"));
    file.line(&format!("import {{ Codec, Encoder, decodeNull, decodeList, decodeObject, encodeNull, encodeList, encodeObject }} from \"{}\"", runtime));
    file.blank_line();

    file.begin_indent("export class SchemaCodec {");
    file.begin_indent("public constructor(");
    file.line("private readonly scalars: Scalars,");
    file.end_indent(") { }");
    file.blank_line();

    for object in inputs {
        file.begin_indent(&format!("public {}: Encoder = {{", object.name));
        for field in &object.fields {
            file.line(&format!("{}: (value) => {},", field.name, encode_to_code(&field.field_type)));
        }
        file.end_indent("}");
    }

    for object in outputs {
        file.begin_indent(&format!("public {}: Codec = {{", object.name));
        for field in &object.fields {
            file.begin_indent(&format!("{}: {{", field.name));

            match resolve_encoding_target(&field.field_type) {
                EncodingTarget::SingleField => (),
                EncodingTarget::Object(name) => {
                    file.line(&format!("codec: () => this.{},", name));
                }
            }
            file.line(&format!("decode: (value) => {},", decode_to_code(&field.field_type)));
            if let GqlType::Function { inputs, .. } = &field.field_type {
                file.begin_indent("args: {");
                for input in inputs {
                    file.begin_indent(&format!("{}: {{", input.name));
                    file.line(&format!("type: \"{}\",", input.type_name));
                    file.line(&format!("encode: (value) => {},", encode_to_code(&input.argument_type)));
                    file.end_indent("},");
                }
                file.end_indent("}");
            }
            file.end_indent("},");
        }
        file.end_indent("}");
    }

    file.end_indent("}");
    file.to_string()
}

fn resolve_encoding_target(gql_type: &GqlType) -> EncodingTarget {
    match gql_type {
        GqlType::Scalar(_) => EncodingTarget::SingleField,
        GqlType::Enum(_) => EncodingTarget::SingleField,
        GqlType::Nullable(inner) => resolve_encoding_target(inner),
        GqlType::List(inner) => resolve_encoding_target(inner),
        GqlType::Object(name) => EncodingTarget::Object(name.to_string()),
        GqlType::Function { output, .. } => resolve_encoding_target(output)
    }
}

enum EncodingTarget {
    SingleField,
    Object(String)
}

fn decode_to_code(gql_type: &GqlType) -> String {
    match gql_type {
        GqlType::Nullable(inner) => format!("decodeNull(value, value => {})", decode_to_code(inner)),
        GqlType::List(inner) => format!("decodeList(value, value => {})", decode_to_code(inner)),
        GqlType::Enum(_) => "value".to_string(),
        GqlType::Scalar(name) => format!("this.scalars.{}.decode(value)", name),
        GqlType::Object(name) => format!("decodeObject(value, this.{})", name),
        GqlType::Function { output, .. } => decode_to_code(output)
    }
}

fn encode_to_code(gql_type: &GqlType) -> String {
    match gql_type {
        GqlType::Nullable(inner) => format!("encodeNull(value, value => {})", encode_to_code(inner)),
        GqlType::List(inner) => format!("encodeList(value, value => {})", encode_to_code(inner)),
        GqlType::Enum(_) => "value".to_string(),
        GqlType::Scalar(name) => format!("this.scalars.{}.encode(value)", name),
        GqlType::Object(name) => format!("encodeObject(value, this.{})", name),
        GqlType::Function { .. } => panic!("Unable to encode argument as function inside function"),
    }
}
