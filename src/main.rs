#[macro_use]
extern crate log;

mod backend;
mod error;
mod utils;

use anyhow::Result;
use clap::Parser;
use error::ServerError;
use hyper::{
    body::HttpBody,
    server::conn::AddrStream,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use once_cell::sync::OnceCell;
use std::{net::SocketAddr, path::PathBuf, sync::Mutex};
use utils::{Graph, LogLevel, Metadata};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

// server info
// pub(crate) static SERVER_INFO: OnceCell<ServerInfo> = OnceCell::new();

// whisper model
pub(crate) static GRAPH: OnceCell<Mutex<Graph>> = OnceCell::new();

pub(crate) const MAX_BUFFER_SIZE: usize = 2usize.pow(14) * 15 + 128;

// default socket address of LlamaEdge API Server instance
const DEFAULT_SOCKET_ADDRESS: &str = "0.0.0.0:8080";

#[derive(Debug, Parser)]
#[command(name = "Whisper API Server", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Whisper API Server")]
struct Cli {
    /// Model name.
    #[arg(short, long, default_value = "default")]
    model_name: String,
    /// Model alias.
    #[arg(long, default_value = "default")]
    model_alias: String,
    /// Audio file.
    #[arg(long, required = true)]
    audio: String,
    /// Socket address of Whisper API server instance
    #[arg(long, default_value = DEFAULT_SOCKET_ADDRESS)]
    socket_addr: String,
}

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

    // parse the command line arguments
    let cli = Cli::parse();

    // log the version of the server
    info!(target: "server_config", "server version: {}", env!("CARGO_PKG_VERSION"));

    // log model name
    info!(target: "server_config", "model_name: {}", &cli.model_name);

    // log model alias
    info!(target: "server_config", "model_alias: {}", &cli.model_alias);

    // log audio file
    info!(target: "server_config", "audio: {}", &cli.audio);

    // create a Metadata instance
    let metadata = Metadata {
        model_alias: cli.model_alias.clone(),
    };

    // create a Graph instance
    let graph = Graph::new(&metadata).map_err(|e| ServerError::Operation(e.to_string()))?;

    // set GRAPH
    GRAPH
        .set(Mutex::new(graph))
        .map_err(|_| ServerError::Operation("Failed to set `GRAPH`.".to_string()))?;

    // socket address
    let addr = cli
        .socket_addr
        .parse::<SocketAddr>()
        .map_err(|e| ServerError::SocketAddr(e.to_string()))?;

    // log socket address
    info!(target: "server_config", "socket_address: {}", addr.to_string());

    let new_service = make_service_fn(move |conn: &AddrStream| {
        // log socket address
        info!(target: "connection", "remote_addr: {}, local_addr: {}", conn.remote_addr().to_string(), conn.local_addr().to_string());

        async move { Ok::<_, Error>(service_fn(handle_request)) }
    });

    let server = Server::bind(&addr).serve(new_service);

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
            let size: u64 = req
                .headers()
                .get("content-length")
                .unwrap()
                .to_str()
                .unwrap()
                .parse()
                .unwrap();

            info!(target: "request", "method: {}, endpoint: {}, http_version: {}, size: {}", method, path, version, size);
        } else {
            info!(target: "request", "method: {}, endpoint: {}, http_version: {}", method, path, version);
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
            let response_body_size: u64 = response.body().size_hint().lower();
            let response_status = status_code.as_u16();
            let response_is_informational = status_code.is_informational();
            let response_is_success = status_code.is_success();
            let response_is_redirection = status_code.is_redirection();
            let response_is_client_error = status_code.is_client_error();
            let response_is_server_error = status_code.is_server_error();

            info!(target: "response", "version: {}, body_size: {}, status: {}, is_informational: {}, is_success: {}, is_redirection: {}, is_client_error: {}, is_server_error: {}", response_version, response_body_size, response_status, response_is_informational, response_is_success, response_is_redirection, response_is_client_error, response_is_server_error);
        } else {
            let response_version = format!("{:?}", response.version());
            let response_body_size: u64 = response.body().size_hint().lower();
            let response_status = status_code.as_u16();
            let response_is_informational = status_code.is_informational();
            let response_is_success = status_code.is_success();
            let response_is_redirection = status_code.is_redirection();
            let response_is_client_error = status_code.is_client_error();
            let response_is_server_error = status_code.is_server_error();

            error!(target: "response", "version: {}, body_size: {}, status: {}, is_informational: {}, is_success: {}, is_redirection: {}, is_client_error: {}, is_server_error: {}", response_version, response_body_size, response_status, response_is_informational, response_is_success, response_is_redirection, response_is_client_error, response_is_server_error);
        }
    }

    Ok(response)
}
