#[macro_use]
extern crate log;

mod backend;
mod error;

use anyhow::Result;
use clap::Parser;
use error::ServerError;
use hyper::{
    body::HttpBody,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::net::TcpListener;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// default port
const DEFAULT_PORT: &str = "8080";

#[derive(Debug, Parser)]
#[command(name = "Whisper API Server", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Whisper API Server")]
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
    /// Port number
    #[arg(long, default_value = DEFAULT_PORT, value_parser = clap::value_parser!(u16))]
    port: u16,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), ServerError> {
    // get the environment variable `RUST_LOG`
    let rust_log = std::env::var("RUST_LOG").unwrap_or_default().to_lowercase();
    let (_, log_level) = match rust_log.is_empty() {
        true => ("stdout", LogLevel::Info),
        false => match rust_log.split_once("=") {
            Some((target, level)) => (target, level.parse().unwrap_or(LogLevel::Info)),
            None => ("stdout", rust_log.parse().unwrap_or(LogLevel::Info)),
        },
    };

    // set global logger
    wasi_logger::Logger::install().expect("failed to install wasi_logger::Logger");
    log::set_max_level(log_level.into());

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

    // create a Metadata instance
    let metadata = llama_core::AudioMetadataBuilder::new(&cli.model_name, &cli.model_alias)
        .enable_plugin_log(true)
        .enable_debug_log(true)
        .build();

    // init the audio context
    llama_core::init_whisper_context(&metadata, &cli.model)
        .map_err(|e| ServerError::Operation(e.to_string()))?;

    let addr = format!("127.0.0.1:{}", cli.port);

    let new_service = make_service_fn(move |conn: &AddrStream| {
        // log socket address
        info!(target: "stdout",
            "remote_addr: {}, local_addr: {}",
            conn.remote_addr().to_string(),
            conn.local_addr().to_string()
        );

        async move { Ok::<_, Error>(service_fn(handle_request)) }
    });

    let tcp_listener = TcpListener::bind(&addr).await.unwrap();
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
