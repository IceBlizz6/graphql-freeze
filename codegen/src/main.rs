use std::io::Read;
use std::path::PathBuf;
use std::collections::HashMap;
use async_std::fs::File;
use async_std::io::ReadExt;
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

#[async_std::main]
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
        panic!("No output directory was given")
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
                        ConfigProfile::PipeResponse => {
                            (FetchMethod::Pipe, ProcessMethod::Introspection)
                        }
                        ConfigProfile::PipeSdl => {
                            (FetchMethod::Pipe, ProcessMethod::Sdl)
                        }
                    }
                }
                None => panic!("No profile named \"{}\"", profile_name)
            }
        } else {
            panic!("No method to fetch schema was provided and default profile is not defined in config file")
        }
    } else {
        panic!("No method to fetch schema was provided")
    };

    let options = CodegenOptions {
        runtime_package,
        indent,
        line_break,
        output_directory: PathBuf::from(output_directory),
        fetch,
        process
    };

    execute(options).await;
}

fn read_config_from_args(args: &Cli) -> Option<CodegenJsonConfig> {
    match &args.config {
        Some(path) => {
            match read_config(path) {
                Some(config) => Some(config),
                None => panic!("Unable to locate config file {}", path)
            }
        },
        None => read_config(DEFAULT_CONFIG_PATH)
    }
}

fn read_config(path: &str) -> Option<CodegenJsonConfig> {
    if std::fs::exists(path).unwrap() {
        let mut file = std::fs::File::open(path).unwrap();
        let mut config_content = String::new();
        file.read_to_string(&mut config_content).unwrap();
        let deserializer = &mut serde_json::Deserializer::from_str(&config_content);
        Some(serde_path_to_error::deserialize(deserializer).unwrap())
    } else {
        None
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
}

fn default_line_break() -> String {
    if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }.to_string()
}

#[derive(Deserialize)]
struct CodegenJsonConfig {
    profiles: Option<HashMap<String, ConfigProfile>>,
    output_directory: Option<String>,
    #[serde(rename = "lineBreak")]
    line_break: Option<String>,
    indent: Option<String>,
    runtime: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "method")]
enum ConfigProfile {
    Endpoint { url: String },
    File { path: String },
    PipeResponse,
    PipeSdl
}

async fn execute(options: CodegenOptions) {
    let raw_content = match options.fetch {
        FetchMethod::Endpoint { url } => read_endpoint(url).await,
        FetchMethod::File { path } => read_file(path).await,
        FetchMethod::Pipe => read_pipe()
    };
    let document: GqlDocument = match options.process {
        ProcessMethod::Introspection => schema_introspection::from_response_body(&raw_content),
        ProcessMethod::Sdl => schema_sdl::from_sdl_string(&raw_content)
    };
    let write_options = CodeFileOptions {
        indent: options.indent,
        line_break: options.line_break
    };
    code_generator::write_files(document, options.output_directory, write_options, &options.runtime_package).await;
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

async fn read_file(path: PathBuf) -> String {
    let mut file = File::open(path).await.unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content);
    content
}

async fn read_endpoint(url: String) -> String {
    let query = include_str!("../resources/introspect.gql");
    let input_body = GraphQLQuery { query: query.to_string() };
    surf::post(url)
        .body_json(&input_body)
        .unwrap()
        .await
        .unwrap()
        .body_string()
        .await
        .unwrap()
}

fn read_pipe() -> String {
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer).unwrap();
    buffer
}

#[derive(Serialize)]
struct GraphQLQuery {
    query: String
}
