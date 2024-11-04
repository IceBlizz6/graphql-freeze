use std::io;
use std::io::Read;
use std::fs;
use std::fs::File;
use std::process;
use std::path::PathBuf;
use std::collections::HashMap;
use clap::Parser;
use serde::{Deserialize, Serialize};
use crate::schema::GqlDocument;
use crate::code_writer::CodeFileOptions;

mod code_generator;
mod code_writer;
mod schema;
mod schema_sdl;
mod schema_introspection;

const DEFAULT_CONFIG_PATH: &'static str = "graphql-freeze.json";
const DEFAULT_RUNTIME: &'static str = "graphql-freeze";
const DEFAULT_INDENT: &'static str = "    ";
const DEFAULT_PROFILE_NAME: &'static str = "default";

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let config = read_config_from_args(&args);

    let runtime_package = config
        .as_ref()
        .and_then(|c| c.runtime.as_ref())
        .map(|r| r.as_str())
        .unwrap_or(DEFAULT_RUNTIME)
        .to_string();

    let indent = config
        .as_ref()
        .and_then(|c| c.indent.as_ref())
        .map(|r| r.as_str())
        .unwrap_or(DEFAULT_INDENT)
        .to_string();

    let line_break = config
        .as_ref()
        .and_then(|c| c.line_break.as_ref())
        .map(|r| r.clone())
        .unwrap_or_else(|| default_line_break());

    let output_directory: String = if let Some(output) = args.output {
        output
    } else if let Some(output_dir) = &config.as_ref().and_then(|c| c.output_directory.as_ref()) {
        output_dir.to_string()
    } else {
        exit_with_error("No output directory was given")
    };

    let (fetch, process): (FetchMethod, ProcessMethod) = if let Some(url) = args.url {
        (FetchMethod::Endpoint { url }, ProcessMethod::Introspection)
    } else if let Some(file) = args.file {
        (FetchMethod::File { path: PathBuf::from(file) }, ProcessMethod::Sdl)
    } else if let Some(config) = &config {
        let profile_name: String = args.profile.unwrap_or(DEFAULT_PROFILE_NAME.to_string());
        if let Some(profiles) = &config.profiles {
            let profile = profiles.get(&profile_name);
            match profile {
                Some(profile) => {
                    match profile {
                        ConfigProfile::Endpoint { url } => {
                            (FetchMethod::Endpoint { url: url.to_string() }, ProcessMethod::Introspection)
                        }
                        ConfigProfile::File { path } => {
                            (FetchMethod::File { path: PathBuf::from(path) }, ProcessMethod::Sdl)
                        }
                        ConfigProfile::PipeIntrospection => {
                            (FetchMethod::Pipe, ProcessMethod::Introspection)
                        }
                        ConfigProfile::PipeSdl => {
                            (FetchMethod::Pipe, ProcessMethod::Sdl)
                        }
                    }
                }
                None => exit_with_error(&format!("No profile named \"{}\"", profile_name))
            }
        } else {
            exit_with_error("No method to fetch schema was provided and default profile is not defined in config file")
        }
    } else {
        exit_with_error("No method to fetch schema was provided, use --url, --file or make a config")
    };

    let options = CodegenOptions {
        runtime_package,
        indent,
        line_break,
        output_directory: PathBuf::from(output_directory),
        fetch,
        process
    };

    execute(options, args.dump_on_parse_error).await;
}

fn read_config_from_args(args: &Cli) -> Option<CodegenJsonConfig> {
    match &args.config {
        Some(path) => {
            match read_config(path) {
                Ok(config) => {
                    match config {
                        Some(config) => Some(config),
                        None => exit_with_error(&format!("Unable to locate config file {}", path))
                    }
                }
                Err(error) => exit_with_error(&error.to_string())
            }
        },
        None => {
            match read_config(DEFAULT_CONFIG_PATH) {
                Ok(config) => config,
                Err(error) => exit_with_error(&error.to_string())
            }
        }
    }
}

fn read_config(path: &str) -> Result<Option<CodegenJsonConfig>, std::io::Error> {
    if fs::exists(path)? {
        let mut file = File::open(path)?;
        let mut config_content = String::new();
        file.read_to_string(&mut config_content)?;
        let deserializer = &mut serde_json::Deserializer::from_str(&config_content);
        match serde_path_to_error::deserialize(deserializer) {
            Ok(result) => Ok(Some(result)),
            Err(error) => {
                eprintln!("Error parsing config file {}", path);
                eprintln!("{}", error.to_string());
                process::exit(1)
            }
        }
    } else {
        Ok(None)
    }
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Path to config file from working directory, default: graphql-freeze.json")]
    config: Option<String>,
    #[arg(short, long, help = "Profile used from config file, default: default")]
    profile: Option<String>,
    #[arg(short, long, help = "Generates client from introspection, override config file")]
    url: Option<String>,
    #[arg(short, long, help = "Generates client from SDL in file, override config file")]
    file: Option<String>,
    #[arg(short, long, help = "Output directory, override config file")]
    output: Option<String>,
    #[arg(short = 'e', long = "errdump", default_value_t = false, help = "Print out the contents to stderr on schema parse error, useful for troubleshooting")]
    dump_on_parse_error: bool
}

fn default_line_break() -> String {
    if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }.to_string()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodegenJsonConfig {
    profiles: Option<HashMap<String, ConfigProfile>>,
    #[serde(rename = "outputDirectory")]
    output_directory: Option<String>,
    #[serde(rename = "lineBreak")]
    line_break: Option<String>,
    indent: Option<String>,
    runtime: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "method")]
enum ConfigProfile {
    #[serde(rename = "endpoint")]
    Endpoint { url: String },
    #[serde(rename = "file")]
    File { path: String },
    #[serde(rename = "pipeIntrospection")]
    PipeIntrospection,
    #[serde(rename = "pipeSdl")]
    PipeSdl
}

async fn execute(options: CodegenOptions, show_schema_on_error: bool) {
    let raw_content = match options.fetch {
        FetchMethod::Endpoint { url } => {
            match read_endpoint(&url).await {
                Ok(response) => response,
                Err(error) => {
                    eprintln!("Networking error {}", error.to_string());
                    process::exit(1)
                }
            }
        },
        FetchMethod::File { path } => {
            match read_file(path).await {
                Ok(file_content) => file_content,
                Err(error) => exit_with_error(&error.to_string())
            }
        },
        FetchMethod::Pipe => read_pipe()
    };
    let document: GqlDocument = match options.process {
        ProcessMethod::Introspection => {
            match schema_introspection::from_response_body(&raw_content) {
                Ok(schema) => schema,
                Err(error) => abort_on_schema_parse_fail(show_schema_on_error, &raw_content, &error.to_string())
            }
        },
        ProcessMethod::Sdl => {
            match schema_sdl::from_sdl_string(&raw_content) {
                Ok(schema) => schema,
                Err(error) => abort_on_schema_parse_fail(show_schema_on_error, &raw_content, &error.to_string())
            }
        }
    };
    let write_options = CodeFileOptions {
        indent: options.indent,
        line_break: options.line_break
    };
    code_generator::write_files(document, options.output_directory, write_options, &options.runtime_package).await;
}

fn abort_on_schema_parse_fail(show_schema_on_error: bool, schema_content: &str, error_string: &str) -> ! {
    eprintln!("{}", error_string);
    if show_schema_on_error {
        eprintln!("Error parsing schema types");
        eprintln!("{}", schema_content);
    } else {
        eprintln!("Error parsing schema, use --errdump to display the attempted schema to parse");
    }
    process::exit(1)
}

struct CodegenOptions {
    output_directory: PathBuf,
    line_break: String,
    indent: String,
    runtime_package: String,
    fetch: FetchMethod,
    process: ProcessMethod
}

enum FetchMethod {
    File { path: PathBuf },
    Endpoint { url: String },
    Pipe,
}

enum ProcessMethod {
    Sdl,
    Introspection
}

async fn read_file(path: PathBuf) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

async fn read_endpoint(url: &str) -> Result<String, reqwest::Error> {
    let query = include_str!("../resources/introspect.gql");
    let input_body = GraphQLQuery { query: query.to_string() };
    let client = reqwest::Client::new();
    let response = client
            .post(url)
            .json(&input_body)
            .send()
            .await?
            .error_for_status()?;
    let response_body = response.text().await?;
    Ok(response_body)
}

fn read_pipe() -> String {
    let mut buffer = String::new();
    match io::stdin().read_to_string(&mut buffer) {
        Ok(_) => buffer,
        Err(error) => {
            eprintln!("Error reading from pipe");
            eprintln!("ERROR: {}", error.to_string());
            process::exit(1)
        }
    }
}

#[derive(Serialize)]
struct GraphQLQuery {
    query: String
}

fn exit_with_error(message: &str) -> ! {
    eprintln!("ERROR: {}", message);
    process::exit(1)
}
