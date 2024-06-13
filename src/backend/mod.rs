pub(crate) mod burn;

use crate::error;
use burn::{audio_transcriptions_handler, files_handler, server_info_handler};
use hyper::{Body, Request, Response};

pub(crate) async fn handle_llama_request(req: Request<Body>) -> Response<Body> {
    match req.uri().path() {
        // "/v1/chat/completions" => ggml::chat_completions_handler(req).await,
        // "/v1/completions" => ggml::completions_handler(req).await,
        // "/v1/models" => ggml::models_handler().await,
        // "/v1/embeddings" => ggml::embeddings_handler(req).await,
        "/v1/files" => files_handler(req).await,
        // "/v1/chunks" => ggml::chunks_handler(req).await,
        "/v1/audio/transcriptions" => audio_transcriptions_handler(req).await,
        "/v1/info" => server_info_handler().await,
        _ => error::invalid_endpoint(req.uri().path()),
    }
}
