pub(crate) mod whisper;

use crate::error;
use hyper::{Body, Request, Response};
use whisper::audio_transcriptions_handler;

pub(crate) async fn handle_llama_request(req: Request<Body>) -> Response<Body> {
    match req.uri().path() {
        "/v1/audio/transcriptions" => audio_transcriptions_handler(req).await,
        _ => error::invalid_endpoint(req.uri().path()),
    }
}
