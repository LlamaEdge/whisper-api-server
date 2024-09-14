use crate::error;
use endpoints::{
    audio::{transcription::TranscriptionRequest, translation::TranslationRequest},
    files::FileObject,
};
use hyper::{body::to_bytes, Body, Method, Request, Response};
use multipart::server::{Multipart, ReadEntry, ReadEntryResult};
use multipart_2021 as multipart;
use std::{
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::Path,
    time::SystemTime,
};

pub(crate) async fn whisper_transcriptions_handler(req: Request<Body>) -> Response<Body> {
    // log
    info!(target: "stdout", "Handling the coming audio transcription request");

    let res = match *req.method() {
        Method::POST => {
            let boundary = "boundary=";

            let boundary = match req.headers().get("content-type").and_then(|ct| {
                let ct = ct.to_str().ok()?;
                let idx = ct.find(boundary)?;
                Some(ct[idx + boundary.len()..].to_string())
            }) {
                Some(boundary) => boundary,
                None => {
                    let err_msg = "Failed to get the boundary from the request.";

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let req_body = req.into_body();
            let body_bytes = match to_bytes(req_body).await {
                Ok(body_bytes) => body_bytes,
                Err(e) => {
                    let err_msg = format!("Fail to read buffer from request body. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let cursor = Cursor::new(body_bytes.to_vec());

            let mut multipart = Multipart::with_body(cursor, boundary);

            // create a transcription request
            let mut request = TranscriptionRequest::default();
            while let ReadEntryResult::Entry(mut field) = multipart.read_entry_mut() {
                match &*field.headers.name {
                    "file" => {
                        let filename = match field.headers.filename {
                            Some(filename) => filename,
                            None => {
                                let err_msg =
                                    "Failed to upload the target file. The filename is not provided.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        if !(filename).to_lowercase().ends_with(".wav") {
                            let err_msg = "The audio file (*.wav) must be have a sample rate of 16k and be single-channel.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }

                        let mut buffer = Vec::new();
                        let size_in_bytes = match field.data.read_to_end(&mut buffer) {
                            Ok(size_in_bytes) => size_in_bytes,
                            Err(e) => {
                                let err_msg = format!("Failed to read the target file. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        // create a unique file id
                        let id = format!("file_{}", uuid::Uuid::new_v4());

                        // log
                        info!(target: "stdout", "file_id: {}, file_name: {}", &id, &filename);

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
                                error!(target: "stdout", "{}", &err_msg);

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
                                    error!(target: "stdout", "{}", &err_msg);

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

                                if let Err(e) = field.data.read_to_string(&mut model) {
                                    let err_msg = format!("Failed to read the model. {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

                                request.model = model;
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the model name. The model field in the request should be a text field.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    "language" => unimplemented!(),
                    "prompt" => unimplemented!(),
                    "response_format" => unimplemented!(),
                    "temperature" => unimplemented!(),
                    "timestamp_granularities" => unimplemented!(),
                    _ => {
                        let err_msg = format!("Invalid field name: {}", &field.headers.name);

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                }
            }

            let obj = match llama_core::audio::audio_transcriptions(request).await {
                Ok(obj) => obj,
                Err(e) => {
                    let err_msg = format!("Failed to transcribe the audio. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            // serialize chat completion object
            let s = match serde_json::to_string(&obj) {
                Ok(s) => s,
                Err(e) => {
                    let err_msg = format!("Failed to serialize transcription object. {}", e);

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

            match result {
                Ok(response) => response,
                Err(e) => {
                    let err_msg = e.to_string();

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    error::internal_server_error(err_msg)
                }
            }
        }
        _ => {
            let err_msg = "Invalid HTTP Method.";

            // log
            error!(target: "stdout", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    info!(target: "stdout", "Send the audio transcription response");

    res
}

pub(crate) async fn whisper_translations_handler(req: Request<Body>) -> Response<Body> {
    // log
    info!(target: "stdout", "Handling the coming audio translation request");

    let res = match *req.method() {
        Method::POST => {
            let boundary = "boundary=";

            let boundary = match req.headers().get("content-type").and_then(|ct| {
                let ct = ct.to_str().ok()?;
                let idx = ct.find(boundary)?;
                Some(ct[idx + boundary.len()..].to_string())
            }) {
                Some(boundary) => boundary,
                None => {
                    let err_msg = "Failed to get the boundary from the request.";

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let req_body = req.into_body();
            let body_bytes = match to_bytes(req_body).await {
                Ok(body_bytes) => body_bytes,
                Err(e) => {
                    let err_msg = format!("Fail to read buffer from request body. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let cursor = Cursor::new(body_bytes.to_vec());

            let mut multipart = Multipart::with_body(cursor, boundary);

            // create a transcription request
            let mut request = TranslationRequest::default();
            while let ReadEntryResult::Entry(mut field) = multipart.read_entry_mut() {
                match &*field.headers.name {
                    "file" => {
                        let filename = match field.headers.filename {
                            Some(filename) => filename,
                            None => {
                                let err_msg =
                                    "Failed to upload the target file. The filename is not provided.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        if !(filename).to_lowercase().ends_with(".wav") {
                            let err_msg = "The audio file (*.wav) must be have a sample rate of 16k and be single-channel.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }

                        let mut buffer = Vec::new();
                        let size_in_bytes = match field.data.read_to_end(&mut buffer) {
                            Ok(size_in_bytes) => size_in_bytes,
                            Err(e) => {
                                let err_msg = format!("Failed to read the target file. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

                        // create a unique file id
                        let id = format!("file_{}", uuid::Uuid::new_v4());

                        // log
                        info!(target: "stdout", "file_id: {}, file_name: {}", &id, &filename);

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
                                error!(target: "stdout", "{}", &err_msg);

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
                                    error!(target: "stdout", "{}", &err_msg);

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
                        if field.is_text() {
                            let mut model = String::new();

                            if let Err(e) = field.data.read_to_string(&mut model) {
                                let err_msg = format!("Failed to read the model. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            request.model = Some(model);
                        }
                    }
                    "prompt" => {
                        match field.is_text() {
                            true => {
                                let mut prompt = String::new();

                                if let Err(e) = field.data.read_to_string(&mut prompt) {
                                    let err_msg = format!("Failed to read the prompt. {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

                                request.prompt = Some(prompt);
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the prompt. The prompt field in the request should be a text field.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    "response_format" => {
                        match field.is_text() {
                            true => {
                                let mut response_format = String::new();

                                if let Err(e) = field.data.read_to_string(&mut response_format) {
                                    let err_msg =
                                        format!("Failed to read the response format. {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

                                request.response_format = Some(response_format);
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the response format. The response format field in the request should be a text field.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    "temperature" => {
                        match field.is_text() {
                            true => {
                                let mut temperature = String::new();

                                if let Err(e) = field.data.read_to_string(&mut temperature) {
                                    let err_msg = format!("Failed to read the temperature. {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

                                match temperature.trim().parse::<f64>() {
                                    Ok(temp) => {
                                        request.temperature = Some(temp);
                                    }
                                    Err(e) => {
                                        let err_msg =
                                            format!("Failed to parse the temperature. {}", e);

                                        // log
                                        error!(target: "stdout", "{}", &err_msg);

                                        return error::internal_server_error(err_msg);
                                    }
                                }
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the temperature. The temperature field in the request should be a text field.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    "language" => {
                        match field.is_text() {
                            true => {
                                let mut language = String::new();

                                if let Err(e) = field.data.read_to_string(&mut language) {
                                    let err_msg = format!("Failed to read the prompt. {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

                                request.language = Some(language);
                            }
                            false => {
                                let err_msg =
                                    "Failed to get the spoken language info. The language field in the request should be a text field.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        }
                    }
                    _ => {
                        let err_msg = format!("Invalid field name: {}", &field.headers.name);

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                }
            }

            info!(target: "stdout", "Request: {:?}", &request);

            let obj = match llama_core::audio::audio_translations(request).await {
                Ok(obj) => obj,
                Err(e) => {
                    let err_msg = format!("Failed to translate the audio. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            // serialize chat completion object
            let s = match serde_json::to_string(&obj) {
                Ok(s) => s,
                Err(e) => {
                    let err_msg = format!("Failed to serialize transcription object. {}", e);

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

            match result {
                Ok(response) => response,
                Err(e) => {
                    let err_msg = e.to_string();

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    error::internal_server_error(err_msg)
                }
            }
        }
        _ => {
            let err_msg = "Invalid HTTP Method.";

            // log
            error!(target: "stdout", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    };

    info!(target: "stdout", "Send the audio translation response");

    res
}
