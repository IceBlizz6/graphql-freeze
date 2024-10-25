use std::io::Read;
use std::path::PathBuf;
use std::collections::HashMap;
use async_std::fs::File;
use async_std::io::ReadExt;
use clap::Parser;
use hyper::body::HttpBody;
use serde::{Deserialize, Serialize};
use crate::schema::GqlDocument;
use crate::code_writer::CodeFileOptions;

mod code_generator;
mod code_writer;
mod schema;
mod schema_sdl;
mod schema_introspection;

#[async_std::main]
async fn main() {
    let args = Cli::parse();
    
    let mut file = std::fs::File::open(args.config).unwrap();
    let mut config_content = String::new();
    file.read_to_string(&mut config_content).unwrap();
    
    let deserializer = &mut serde_json::Deserializer::from_str(&config_content);
    let config: CodegenJsonConfig = serde_path_to_error::deserialize(deserializer).unwrap();
    let profile = config.profiles.get(&args.profile);
    
    let (fetch, process): (FetchMethod, ProcessMethod) = match profile {
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
        None => panic!("No profile named \"{}\"", args.profile)
    };
    
    let options = CodegenOptions {
        runtime_package: config.runtime.unwrap_or_else(|| "graphql-freeze".to_string()),
        indent: config.indent.unwrap_or_else(|| "    ".to_string()),
        line_break: config.line_break.unwrap_or_else(|| default_line_break()),
        output_directory: PathBuf::from(config.output),
        fetch,
        process
    };
    
    execute(options).await;
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value_t = String::from("graphql-freeze.json"), help = "Path to json configuration file, defaults to \"graphql-freeze.json\" in working dir")]
    config: String,
    #[arg(short, long, default_value_t = String::from("default"), help = "Profile used from config file, defaults to \"default\"")]
    profile: String
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
    profiles: HashMap<String, ConfigProfile>,
    output: String,
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
    let client = hyper::Client::new();
    let input_body_content = serde_json::to_string(&input_body).unwrap();
    let request = hyper::Request::builder()
        .uri(url)
        .body(hyper::Body::from(input_body_content))
        .unwrap();
    let response = client.request(request).await.unwrap();
    let mut output_body = response.into_body();
    let body_bytes = (&mut output_body).data().await.unwrap().unwrap();
    return String::from_utf8(body_bytes.to_vec()).unwrap();
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
