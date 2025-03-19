#[macro_use]
extern crate log;

mod backend;
mod error;

use anyhow::Result;
use clap::{ArgGroup, Parser, ValueEnum};
use error::ServerError;
use hyper::{
    body::HttpBody,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// server info
pub(crate) static SERVER_INFO: OnceCell<ApiServer> = OnceCell::new();

// default port
const DEFAULT_PORT: &str = "8080";

// server info
pub(crate) static TASK: OnceCell<TaskType> = OnceCell::new();
// API key
pub(crate) static LLAMA_API_KEY: OnceCell<String> = OnceCell::new();
// Use audio pre-processor
pub(crate) static USE_AUDIO_PREPROCESSOR: OnceCell<bool> = OnceCell::new();

#[derive(Debug, Parser)]
#[command(name = "Whisper API Server", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Whisper API Server")]
#[command(group = ArgGroup::new("socket_address_group").multiple(false).args(&["socket_addr", "port"]))]
struct Cli {
    /// Model name.
    #[arg(short = 'n', long, default_value = "default")]
    model_name: String,
    /// Model alias.
    #[arg(short = 'a', long, default_value = "default")]
    model_alias: String,
    /// Path to the whisper model file
    #[arg(short = 'm', long)]
    model: PathBuf,
    /// Number of threads to use during computation
    #[arg(long, default_value = "4")]
    threads: u64,
    /// Number of processors to use during computation
    #[arg(long, default_value = "1")]
    processors: u32,
    /// Task type.
    #[arg(long, default_value = "full")]
    task: TaskType,
    /// Do not pre-process input audio files.
    #[arg(long, default_value = "false")]
    no_audio_preprocessor: bool,
    /// Port number
    #[arg(long, default_value = DEFAULT_PORT, value_parser = clap::value_parser!(u16), group = "socket_address_group")]
    port: u16,
    /// Socket address of LlamaEdge API Server instance. For example, `0.0.0.0:8080`.
    #[arg(long, default_value = None, value_parser = clap::value_parser!(SocketAddr), group = "socket_address_group")]
    socket_addr: Option<SocketAddr>,
}

#[allow(clippy::needless_return)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), ServerError> {
    // get the environment variable `LLAMA_LOG`
    let log_level: LogLevel = std::env::var("LLAMA_LOG")
        .unwrap_or("info".to_string())
        .parse()
        .unwrap_or(LogLevel::Info);

    // set global logger
    wasi_logger::Logger::install().expect("failed to install wasi_logger::Logger");
    log::set_max_level(log_level.into());

    info!(target: "stdout", "log_level: {}", log_level);

    if let Ok(api_key) = std::env::var("API_KEY") {
        // define a const variable for the API key
        if let Err(e) = LLAMA_API_KEY.set(api_key) {
            let err_msg = format!("Failed to set API key. {}", e);

            error!(target: "stdout", "{}", err_msg);

            return Err(ServerError::Operation(err_msg));
        }
    }

    // parse the command line arguments
    let cli = Cli::parse();

    // log the version of the server
    info!(target: "stdout", "Whisper API Server v{}", env!("CARGO_PKG_VERSION"));

    // log model name
    info!(target: "stdout", "model name: {}", &cli.model_name);

    // log model alias
    info!(target: "stdout", "model alias: {}", &cli.model_alias);

    // log model path
    info!(target: "stdout", "model path: {}", cli.model.display());

    // log the number of threads
    info!(target: "stdout", "threads: {}", cli.threads);

    // log the number of processors
    info!(target: "stdout", "processors: {}", cli.processors);

    // log the task type
    info!(target: "stdout", "task: {}", cli.task);

    TASK.set(cli.task)
        .map_err(|_| ServerError::Operation("Failed to set `TASK`.".to_string()))?;

    info!(target: "stdout", "pre-process input audio files: {}", !cli.no_audio_preprocessor);

    USE_AUDIO_PREPROCESSOR
        .set(!cli.no_audio_preprocessor)
        .map_err(|_| {
            ServerError::Operation("Failed to set `USE_AUDIO_PREPROCESSOR`.".to_string())
        })?;

    // create a Metadata instance
    let metadata = llama_core::metadata::whisper::WhisperMetadataBuilder::new(
        &cli.model_name,
        &cli.model_alias,
    )
    .with_model_path(&cli.model)
    .enable_plugin_log(true)
    .enable_debug_log(true)
    .build();

    // init the audio context
    llama_core::init_whisper_context(&metadata)
        .map_err(|e| ServerError::Operation(e.to_string()))?;
    let mut translate_model = None;
    let mut transcribe_model = None;
    match cli.task {
        TaskType::Transcriptions => {
            transcribe_model = Some(ModelConfig {
                name: cli.model_name,
                ty: "transcribe".to_string(),
            });
        }
        TaskType::Translations => {
            translate_model = Some(ModelConfig {
                name: cli.model_name,
                ty: "translate".to_string(),
            });
        }
        TaskType::Full => {
            translate_model = Some(ModelConfig {
                name: cli.model_name.clone(),
                ty: "translate".to_string(),
            });
            transcribe_model = Some(ModelConfig {
                name: cli.model_name.clone(),
                ty: "transcribe".to_string(),
            });
        }
    };

    // socket address
    let addr = match cli.socket_addr {
        Some(addr) => addr,
        None => SocketAddr::from(([0, 0, 0, 0], cli.port)),
    };

    // create server info
    let server_info = ApiServer {
        ty: "whisper".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        port: addr.port().to_string(),
        translate_model,
        transcribe_model,
        extras: HashMap::new(),
    };
    SERVER_INFO
        .set(server_info)
        .map_err(|_| ServerError::Operation("Failed to set `SERVER_INFO`.".to_string()))?;

    let new_service = make_service_fn(move |conn: &AddrStream| {
        // log socket address
        info!(target: "stdout",
            "remote_addr: {}, local_addr: {}",
            conn.remote_addr().to_string(),
            conn.local_addr().to_string()
        );

        async move { Ok::<_, Error>(service_fn(handle_request)) }
    });

    let tcp_listener = TcpListener::bind(addr).await.unwrap();
    info!(target: "stdout", "Listening on {}", addr);

    let server = Server::from_tcp(tcp_listener.into_std().unwrap())
        .unwrap()
        .serve(new_service);

    match server.await {
        Ok(_) => Ok(()),
        Err(e) => Err(ServerError::Operation(e.to_string())),
    }
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let path_str = req.uri().path();
    let path_buf = PathBuf::from(path_str);
    let mut path_iter = path_buf.iter();
    path_iter.next(); // Must be Some(OsStr::new(&path::MAIN_SEPARATOR.to_string()))
    let root_path = path_iter.next().unwrap_or_default();
    let root_path = "/".to_owned() + root_path.to_str().unwrap_or_default();

    // check if the API key is valid
    if let Some(auth_header) = req.headers().get("authorization") {
        if !auth_header.is_empty() {
            let auth_header = match auth_header.to_str() {
                Ok(auth_header) => auth_header,
                Err(e) => {
                    let err_msg = format!("Failed to get authorization header: {}", e);
                    return Ok(error::unauthorized(err_msg));
                }
            };

            let api_key = auth_header.split(" ").nth(1).unwrap_or_default();
            info!(target: "stdout", "API Key: {}", api_key);

            if let Some(stored_api_key) = LLAMA_API_KEY.get() {
                if api_key != stored_api_key {
                    let err_msg = "Invalid API key.";
                    return Ok(error::unauthorized(err_msg));
                }
            }
        }
    }

    // log request
    {
        let method = hyper::http::Method::as_str(req.method()).to_string();
        let path = req.uri().path().to_string();
        let version = format!("{:?}", req.version());
        if req.method() == hyper::http::Method::POST {
            let size: u64 = match req.headers().get("content-length") {
                Some(content_length) => content_length.to_str().unwrap().parse().unwrap(),
                None => 0,
            };

            info!(target: "stdout", "method: {}, http_version: {}, content-length: {}", method, version, size);
            info!(target: "stdout", "endpoint: {}", path);
        } else {
            info!(target: "stdout", "method: {}, http_version: {}", method, version);
            info!(target: "stdout", "endpoint: {}", path);
        }
    }

    let response = match root_path.as_str() {
        "/echo" => Response::new(Body::from("echo test")),
        "/v1" => backend::handle_llama_request(req).await,
        _ => error::invalid_endpoint("The requested service endpoint is not found."),
    };

    // log response
    {
        let status_code = response.status();
        if status_code.as_u16() < 400 {
            // log response
            let response_version = format!("{:?}", response.version());
            info!(target: "stdout", "response_version: {}", response_version);
            let response_body_size: u64 = response.body().size_hint().lower();
            info!(target: "stdout", "response_body_size: {}", response_body_size);
            let response_status = status_code.as_u16();
            info!(target: "stdout", "response_status: {}", response_status);
            let response_is_success = status_code.is_success();
            info!(target: "stdout", "response_is_success: {}", response_is_success);
        } else {
            let response_version = format!("{:?}", response.version());
            error!(target: "stdout", "response_version: {}", response_version);
            let response_body_size: u64 = response.body().size_hint().lower();
            error!(target: "stdout", "response_body_size: {}", response_body_size);
            let response_status = status_code.as_u16();
            error!(target: "stdout", "response_status: {}", response_status);
            let response_is_success = status_code.is_success();
            error!(target: "stdout", "response_is_success: {}", response_is_success);
            let response_is_client_error = status_code.is_client_error();
            error!(target: "stdout", "response_is_client_error: {}", response_is_client_error);
            let response_is_server_error = status_code.is_server_error();
            error!(target: "stdout", "response_is_server_error: {}", response_is_server_error);
        }
    }

    Ok(response)
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogLevel {
    /// Describes messages about the values of variables and the flow of
    /// control within a program.
    Trace,

    /// Describes messages likely to be of interest to someone debugging a
    /// program.
    Debug,

    /// Describes messages likely to be of interest to someone monitoring a
    /// program.
    Info,

    /// Describes messages indicating hazardous situations.
    Warn,

    /// Describes messages indicating serious errors.
    Error,

    /// Describes messages indicating fatal errors.
    Critical,
}
impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Critical => log::LevelFilter::Error,
        }
    }
}
impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
            LogLevel::Critical => write!(f, "critical"),
        }
    }
}
impl std::str::FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            "critical" => Ok(LogLevel::Critical),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// Task type.
#[derive(Clone, Debug, Copy, PartialEq, Eq, ValueEnum)]
enum TaskType {
    /// `tracriptions` task.
    #[value(name = "transcribe")]
    Transcriptions,
    /// `translations` task.
    #[value(name = "translate")]
    Translations,
    /// `transcriptions` and `translations` tasks.
    #[value(name = "full")]
    Full,
}
impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TaskType::Transcriptions => write!(f, "transcriptions"),
            TaskType::Translations => write!(f, "translations"),
            TaskType::Full => write!(f, "transcriptions and translations"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ApiServer {
    #[serde(rename = "type")]
    ty: String,
    version: String,
    port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    translate_model: Option<ModelConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcribe_model: Option<ModelConfig>,
    extras: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ModelConfig {
    // model name
    name: String,
    // type: chat or embedding
    #[serde(rename = "type")]
    ty: String,
}
