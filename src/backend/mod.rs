pub(crate) mod whisper;

use crate::{error, TaskType, TASK};
use hyper::{Body, Request, Response};
use whisper::{whisper_transcriptions_handler, whisper_translations_handler};

pub(crate) async fn handle_llama_request(req: Request<Body>) -> Response<Body> {
    // get task
    let task = match TASK.get() {
        Some(task) => task,
        None => {
            let err_msg = "The task is not set.";

            // log
            error!(target: "stdout", "{}", &err_msg);

            return error::internal_server_error("The task is not set.");
        }
    };

    match req.uri().path() {
        "/v1/audio/transcriptions" => match task {
            TaskType::Full | TaskType::Transcriptions => whisper_transcriptions_handler(req).await,
            _ => {
                let err_msg = "The current API server only support transcription tasks. To support translation and/or transcription tasks, please restart the API server with `--task full` or `--task translate`.";

                // log
                error!(target: "stdout", "{}", &err_msg);

                error::internal_server_error(err_msg)
            }
        },
        "/v1/audio/translations" => match task {
            TaskType::Full | TaskType::Translations => whisper_translations_handler(req).await,
            _ => {
                let err_msg = "The current API server only support translation tasks. To support transcription and/or translation tasks, please restart the API server with `--task full` or `--task transcribe`.";

                // log
                error!(target: "stdout", "{}", &err_msg);

                error::internal_server_error(err_msg)
            }
        },
        "/v1/models" => whisper::models_handler().await,
        "/v1/info" => whisper::server_info_handler().await,
        "/v1/files" => whisper::files_handler(req).await,
        path => {
            if path.starts_with("/v1/files/") {
                whisper::files_handler(req).await
            } else {
                error::invalid_endpoint(path)
            }
        }
    }
}
