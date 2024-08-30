use crate::error;
use endpoints::audio::speech::SpeechRequest;
use hyper::{body::to_bytes, http::Method, Body, Request, Response};

pub(crate) async fn audio_speech_handler(req: Request<Body>) -> Response<Body> {
    // log
    info!(target: "stdout", "Handling the coming audio speech request");

    if req.method().eq(&Method::OPTIONS) {
        let result = Response::builder()
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "*")
            .header("Access-Control-Allow-Headers", "*")
            .header("Content-Type", "application/json")
            .body(Body::empty());

        match result {
            Ok(response) => return response,
            Err(e) => {
                let err_msg = e.to_string();

                // log
                error!(target: "stdout", "{}", &err_msg);

                return error::internal_server_error(err_msg);
            }
        }
    }

    info!(target: "stdout", "Prepare the chat completion request.");

    // parse request
    let body_bytes = match to_bytes(req.into_body()).await {
        Ok(body_bytes) => body_bytes,
        Err(e) => {
            let err_msg = format!("Fail to read buffer from request body. {}", e);

            // log
            error!(target: "stdout", "{}", &err_msg);

            return error::internal_server_error(err_msg);
        }
    };
    let speech_request: SpeechRequest = match serde_json::from_slice(&body_bytes) {
        Ok(speech_request) => speech_request,
        Err(e) => {
            let err_msg = format!("Fail to deserialize speech request: {msg}", msg = e);

            // log
            error!(target: "stdout", "{}", &err_msg);

            return error::bad_request(err_msg);
        }
    };

    let file_obj = match llama_core::audio::create_speech(speech_request).await {
        Ok(obj) => obj,
        Err(e) => {
            let err_msg = format!("Failed to transcribe the audio. {}", e);

            // log
            error!(target: "stdout", "{}", &err_msg);

            return error::internal_server_error(err_msg);
        }
    };

    // serialize the file object
    let s = match serde_json::to_string(&file_obj) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Failed to serialize the file object. {}", e);

            // log
            error!(target: "stdout", "{}", &err_msg);

            return error::internal_server_error(err_msg);
        }
    };

    // return response
    let result = Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Content-Type", "application/json")
        .body(Body::from(s));

    let res = match result {
        Ok(response) => response,
        Err(e) => {
            let err_msg = e.to_string();

            // log
            error!(target: "stdout", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    info!(target: "stdout", "Send the audio speech response");

    res
}
