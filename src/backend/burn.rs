use crate::{error, SERVER_INFO};
use endpoints::{
    audio::{TranscriptionObject, TranscriptionRequest},
    files::FileObject,
};
use futures_util::TryStreamExt;
use hyper::{body::to_bytes, Body, Method, Request, Response};
use multipart::server::{Multipart, ReadEntry, ReadEntryResult};
use multipart_2021 as multipart;
use std::{
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::Path,
    time::SystemTime,
};

pub(crate) async fn audio_transcriptions_handler(req: Request<Body>) -> Response<Body> {
    // log
    info!(target: "audio_transcriptions_handler", "Handling the coming audio transcription request");

    let res = match *req.method() {
        Method::POST => {
            let boundary = "boundary=";

            let boundary = req.headers().get("content-type").and_then(|ct| {
                let ct = ct.to_str().ok()?;
                let idx = ct.find(boundary)?;
                Some(ct[idx + boundary.len()..].to_string())
            });

            let req_body = req.into_body();
            let body_bytes = match to_bytes(req_body).await {
                Ok(body_bytes) => body_bytes,
                Err(e) => {
                    let err_msg = format!("Fail to read buffer from request body. {}", e);

                    // log
                    error!(target: "files_handler", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let cursor = Cursor::new(body_bytes.to_vec());

            let mut multipart = Multipart::with_body(cursor, boundary.unwrap());

            let mut request = TranscriptionRequest::default();
            // let mut file_object: Option<FileObject> = None;
            while let ReadEntryResult::Entry(mut field) = multipart.read_entry_mut() {
                match &*field.headers.name {
                    "file" => {
                        let filename = match field.headers.filename {
                            Some(filename) => filename,
                            None => {
                                let err_msg =
                                    "Failed to upload the target file. The filename is not provided.";

                                // log
                                error!(target: "files_handler", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        if !((filename).to_lowercase().ends_with(".mp3")
                            || (filename).to_lowercase().ends_with(".mp4")
                            || (filename).to_lowercase().ends_with(".mpeg")
                            || (filename).to_lowercase().ends_with(".mpga")
                            || (filename).to_lowercase().ends_with(".m4a")
                            || (filename).to_lowercase().ends_with(".wav")
                            || (filename).to_lowercase().ends_with(".webm"))
                        {
                            let err_msg = "Failed to upload the target audio file. File uploads are currently limited to 25 MB and the following input file types are supported: mp3, mp4, mpeg, mpga, m4a, wav, and webm.";

                            // log
                            error!(target: "audio_transcriptions_handler", "{}", err_msg);

                            return error::internal_server_error(err_msg);
                        }

                        let mut buffer = Vec::new();
                        let size_in_bytes = match field.data.read_to_end(&mut buffer) {
                            Ok(size_in_bytes) => size_in_bytes,
                            Err(e) => {
                                let err_msg = format!("Failed to read the target file. {}", e);

                                // log
                                error!(target: "files_handler", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        // create a unique file id
                        let id = format!("file_{}", uuid::Uuid::new_v4());

                        // log
                        info!(target: "audio_transcriptions_handler", "file_id: {}, file_name: {}", &id, &filename);

                        // save the file
                        let path = Path::new("archives");
                        if !path.exists() {
                            fs::create_dir(path).unwrap();
                        }
                        let file_path = path.join(&id);
                        if !file_path.exists() {
                            fs::create_dir(&file_path).unwrap();
                        }
                        let mut file = match File::create(file_path.join(&filename)) {
                            Ok(file) => file,
                            Err(e) => {
                                let err_msg = format!(
                                    "Failed to create archive document {}. {}",
                                    &filename, e
                                );

                                // log
                                error!(target: "files_handler", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };
                        file.write_all(&buffer[..]).unwrap();

                        let created_at =
                            match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                                Ok(n) => n.as_secs(),
                                Err(_) => {
                                    let err_msg = "Failed to get the current time.";

                                    // log
                                    error!(target: "files_handler", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }
                            };

                        // create a file object
                        request.file = FileObject {
                            id,
                            bytes: size_in_bytes as u64,
                            created_at,
                            filename,
                            object: "file".to_string(),
                            purpose: "assistants".to_string(),
                        };
                    }
                    "model" => {
                        match field.is_text() {
                            true => {
                                let mut model = String::new();
                                let size = match field.data.read_to_string(&mut model) {
                                    Ok(size) => size,
                                    Err(e) => {
                                        let err_msg = format!("Failed to read the model. {}", e);

                                        // log
                                        error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                                        return error::internal_server_error(err_msg);
                                    }
                                };

                                request.model = model;
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the model name. The model field in the request should be a text field.";

                                // log
                                error!(target: "audio_transcriptions_handler", "{}", err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    "language" => unimplemented!(),
                    "prompt" => unimplemented!(),
                    "response_format" => unimplemented!(),
                    "temerature" => unimplemented!(),
                    "timestamp_granularities" => unimplemented!(),
                    _ => unimplemented!(),
                }
            }

            // log
            info!(target: "audio_transcriptions_handler", "audio transcription request: {:?}", &request);

            // TODO: call the transcription service

            let obj = TranscriptionObject {
                text: "This is a test".to_string(),
            };

            // serialize chat completion object
            let s = match serde_json::to_string(&obj) {
                Ok(s) => s,
                Err(e) => {
                    let err_msg = format!("Failed to serialize transcription object. {}", e);

                    // log
                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

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

            match result {
                Ok(response) => response,
                Err(e) => {
                    let err_msg = e.to_string();

                    // log
                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                    error::internal_server_error(err_msg)
                }
            }
        }
        _ => {
            let err_msg = "Invalid HTTP Method.";

            // log
            error!(target: "files_handler", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    info!(target: "audio_transcriptions_handler", "Send the audio transcription response");

    res
}

pub(crate) async fn server_info_handler() -> Response<Body> {
    // log
    info!(target: "server_info", "Handling the coming server info request.");

    // get the server info
    let server_info = match SERVER_INFO.get() {
        Some(server_info) => server_info,
        None => {
            let err_msg = "The server info is not set.";

            // log
            error!(target: "server_info_handler", "{}", &err_msg);

            return error::internal_server_error("The server info is not set.");
        }
    };

    // serialize server info
    let s = match serde_json::to_string(&server_info) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Fail to serialize server info. {}", e);

            // log
            error!(target: "server_info_handler", "{}", &err_msg);

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
            error!(target: "server_info_handler", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    info!(target: "server_info", "Send the server info response.");

    res
}
