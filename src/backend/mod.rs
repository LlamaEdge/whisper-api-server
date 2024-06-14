pub(crate) mod burn;

use crate::error;
use burn::audio_transcriptions_handler;
use hyper::{Body, Request, Response};

pub(crate) async fn handle_llama_request(req: Request<Body>) -> Response<Body> {
    match req.uri().path() {
        "/v1/audio/transcriptions" => audio_transcriptions_handler(req).await,
        _ => error::invalid_endpoint(req.uri().path()),
    }
}
