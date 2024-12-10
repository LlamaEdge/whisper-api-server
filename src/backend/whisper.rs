use crate::{error, METADATA};
use endpoints::{
    audio::{transcription::TranscriptionRequest, translation::TranslationRequest},
    files::{DeleteFileStatus, FileObject},
};
use hyper::{body::to_bytes, Body, Method, Request, Response};
use multipart::server::{Multipart, ReadEntry, ReadEntryResult};
use multipart_2021 as multipart;
use std::{
    fs,
    io::{Cursor, Read},
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
                        let mut filename = match field.headers.filename {
                            Some(filename) => filename,
                            None => {
                                let err_msg =
                                    "Failed to upload the target file. The filename is not provided.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

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

                        // create wav audio file to store the converted audio data
                        let path = Path::new("archives");
                        if !path.exists() {
                            fs::create_dir(path).unwrap();
                        }
                        let file_path = path.join(&id);
                        if !file_path.exists() {
                            fs::create_dir(&file_path).unwrap();
                        }
                        let output_file = file_path.join(&filename);
                        let output_wav_file = output_file.with_extension("wav");
                        filename = output_wav_file
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();

                        // log
                        info!(target: "stdout", "file_id: {}, file_name: {}", &id, &filename);

                        info!(target: "stdout", "Pre-processing the audio file...");

                        // create a audio converter
                        let converter = wavup::AudioConverterBuilder::new(
                            output_wav_file.to_string_lossy(),
                            llama_core::metadata::whisper::WHISPER_SAMPLE_RATE as u32,
                        )
                        .build();

                        // convert to a wav audio file with the given sample rate
                        if let Err(e) = converter.convert_audio_from_bytes(&buffer) {
                            let err_msg = format!("Failed to convert audio. {}", e);
                            error!(target: "stdout", "{}", &err_msg);
                            return error::internal_server_error(err_msg);
                        }

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

                                request.model = Some(model);
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
                    "language" => match field.is_text() {
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
                    },
                    "prompt" => match field.is_text() {
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
                    },
                    "response_format" => unimplemented!(),
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
                    "timestamp_granularities" => unimplemented!(),
                    "detect_language" => match field.is_text() {
                        true => {
                            let mut detect_language: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut detect_language) {
                                let err_msg = format!("Failed to read `detect_language`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match detect_language.parse::<bool>() {
                                Ok(detect_language) => {
                                    request.detect_language = Some(detect_language)
                                }
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `detect_language`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `detect_language`. The `detect_language` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "offset_time" => match field.is_text() {
                        true => {
                            let mut offset_time: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut offset_time) {
                                let err_msg = format!("Failed to read `offset_time`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match offset_time.parse::<u64>() {
                                Ok(offset_time) => request.offset_time = Some(offset_time),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `offset_time`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `offset_time`. The `offset_time` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "duration" => match field.is_text() {
                        true => {
                            let mut duration: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut duration) {
                                let err_msg = format!("Failed to read `duration`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match duration.parse::<u64>() {
                                Ok(duration) => request.duration = Some(duration),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `duration`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `duration`. The `duration` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "max_context" => match field.is_text() {
                        true => {
                            let mut max_context: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut max_context) {
                                let err_msg = format!("Failed to read `max_context`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match max_context.parse::<i32>() {
                                Ok(max_context) => request.max_context = Some(max_context),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `max_context`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `max_context`. The `max_context` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "max_len" => match field.is_text() {
                        true => {
                            let mut max_len: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut max_len) {
                                let err_msg = format!("Failed to read `max_len`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match max_len.parse::<u64>() {
                                Ok(max_len) => request.max_len = Some(max_len),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `max_len`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `max_len`. The `max_len` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "split_on_word" => match field.is_text() {
                        true => {
                            let mut split_on_word: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut split_on_word) {
                                let err_msg = format!("Failed to read `split_on_word`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match split_on_word.parse::<bool>() {
                                Ok(split_on_word) => request.split_on_word = Some(split_on_word),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `split_on_word`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `split_on_word`. The `split_on_word` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "use_new_context" => match field.is_text() {
                        true => {
                            let mut use_new_context: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut use_new_context) {
                                let err_msg = format!("Failed to read `use_new_context`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match use_new_context.parse::<bool>() {
                                Ok(use_new_context) => {
                                    request.use_new_context = use_new_context;
                                }
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `use_new_context`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `use_new_context`. The `use_new_context` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    _ => {
                        let err_msg = format!("Invalid field name: {}", &field.headers.name);

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                }
            }

            if Some(true) == request.detect_language {
                request.language = Some("auto".to_string());
            }

            info!(target: "stdout", "Request: {:?}", &request);

            // check if the request uses a new whisper computation context
            if request.use_new_context {
                info!(target: "stdout", "Create a new Whisper computation context");

                let metadata = match METADATA.get() {
                    Some(metadata) => metadata,
                    None => {
                        let err_msg = "Failed to get `METADATA`.";

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                };

                // init the audio context
                if let Err(e) = llama_core::init_whisper_context(metadata) {
                    let err_msg =
                        format!("Failed to create a new Whisper computation context. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
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
                        let mut filename = match field.headers.filename {
                            Some(filename) => filename,
                            None => {
                                let err_msg =
                                    "Failed to upload the target file. The filename is not provided.";

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }
                        };

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

                        // create wav audio file to store the converted audio data
                        let path = Path::new("archives");
                        if !path.exists() {
                            fs::create_dir(path).unwrap();
                        }
                        let file_path = path.join(&id);
                        if !file_path.exists() {
                            fs::create_dir(&file_path).unwrap();
                        }
                        let output_file = file_path.join(&filename);
                        let output_wav_file = output_file.with_extension("wav");
                        filename = output_wav_file
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();

                        // log
                        info!(target: "stdout", "file_id: {}, file_name: {}", &id, &filename);

                        // create a audio converter
                        let converter = wavup::AudioConverterBuilder::new(
                            output_wav_file.to_string_lossy(),
                            llama_core::metadata::whisper::WHISPER_SAMPLE_RATE as u32,
                        )
                        .build();

                        // convert to a wav audio file with the given sample rate
                        if let Err(e) = converter.convert_audio_from_bytes(&buffer) {
                            let err_msg = format!("Failed to convert audio. {}", e);
                            error!(target: "stdout", "{}", &err_msg);
                            return error::internal_server_error(err_msg);
                        }

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
                    "detect_language" => match field.is_text() {
                        true => {
                            let mut detect_language: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut detect_language) {
                                let err_msg = format!("Failed to read `detect_language`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match detect_language.parse::<bool>() {
                                Ok(detect_language) => {
                                    request.detect_language = Some(detect_language)
                                }
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `detect_language`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `detect_language`. The `detect_language` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "offset_time" => match field.is_text() {
                        true => {
                            let mut offset_time: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut offset_time) {
                                let err_msg = format!("Failed to read `offset_time`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match offset_time.parse::<u64>() {
                                Ok(offset_time) => request.offset_time = Some(offset_time),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `offset_time`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `offset_time`. The `offset_time` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "duration" => match field.is_text() {
                        true => {
                            let mut duration: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut duration) {
                                let err_msg = format!("Failed to read `duration`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match duration.parse::<u64>() {
                                Ok(duration) => request.duration = Some(duration),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `duration`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `duration`. The `duration` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "max_context" => match field.is_text() {
                        true => {
                            let mut max_context: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut max_context) {
                                let err_msg = format!("Failed to read `max_context`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match max_context.parse::<i32>() {
                                Ok(max_context) => request.max_context = Some(max_context),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `max_context`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `max_context`. The `max_context` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "max_len" => match field.is_text() {
                        true => {
                            let mut max_len: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut max_len) {
                                let err_msg = format!("Failed to read `max_len`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match max_len.parse::<u64>() {
                                Ok(max_len) => request.max_len = Some(max_len),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `max_len`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `max_len`. The `max_len` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "split_on_word" => match field.is_text() {
                        true => {
                            let mut split_on_word: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut split_on_word) {
                                let err_msg = format!("Failed to read `split_on_word`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match split_on_word.parse::<bool>() {
                                Ok(split_on_word) => request.split_on_word = Some(split_on_word),
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `split_on_word`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `split_on_word`. The `split_on_word` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    "use_new_context" => match field.is_text() {
                        true => {
                            let mut use_new_context: String = String::new();

                            if let Err(e) = field.data.read_to_string(&mut use_new_context) {
                                let err_msg = format!("Failed to read `use_new_context`. {}", e);

                                // log
                                error!(target: "stdout", "{}", &err_msg);

                                return error::internal_server_error(err_msg);
                            }

                            match use_new_context.parse::<bool>() {
                                Ok(use_new_context) => {
                                    request.use_new_context = use_new_context;
                                }
                                Err(e) => {
                                    let err_msg =
                                        format!("Failed to parse `use_new_context`. Reason: {}", e);

                                    // log
                                    error!(target: "stdout", "{}", &err_msg);

                                    return error::bad_request(err_msg);
                                }
                            }
                        }
                        false => {
                            let err_msg =
                                "Failed to get `use_new_context`. The `use_new_context` field in the request should be a text field.";

                            // log
                            error!(target: "stdout", "{}", &err_msg);

                            return error::internal_server_error(err_msg);
                        }
                    },
                    _ => {
                        let err_msg = format!("Invalid field name: {}", &field.headers.name);

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                }
            }

            if Some(true) == request.detect_language {
                request.language = Some("auto".to_string());
            }

            info!(target: "stdout", "Request: {:?}", &request);

            // check if the request uses a new whisper computation context
            if request.use_new_context {
                info!(target: "stdout", "Create a new Whisper computation context");

                let metadata = match METADATA.get() {
                    Some(metadata) => metadata,
                    None => {
                        let err_msg = "Failed to get `METADATA`.";

                        // log
                        error!(target: "stdout", "{}", &err_msg);

                        return error::internal_server_error(err_msg);
                    }
                };

                // init the audio context
                if let Err(e) = llama_core::init_whisper_context(metadata) {
                    let err_msg =
                        format!("Failed to create a new Whisper computation context. {}", e);

                    // log
                    error!(target: "stdout", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            }

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

/// List all files, or remove a file by id.
///
/// - `GET /v1/files`: List all files.
/// - `DELETE /v1/files/{file_id}`: Delete a file by id.
///
pub(crate) async fn files_handler(req: Request<Body>) -> Response<Body> {
    // log
    info!(target: "stdout", "Handling the coming files request");

    let res = if req.method() == Method::GET {
        let uri_path = req.uri().path().trim_end_matches('/').to_lowercase();

        // Split the path into segments
        let segments: Vec<&str> = uri_path.split('/').collect();

        match segments.as_slice() {
            ["", "v1", "files"] => list_files(),
            _ => {
                let err_msg = format!("unsupported uri path: {}", uri_path);

                // log
                error!(target: "stdout", "{}", &err_msg);

                error::internal_server_error(err_msg)
            }
        }
    } else if req.method() == Method::DELETE {
        let id = req.uri().path().trim_start_matches("/v1/files/");
        let status = match llama_core::files::remove_file(id) {
            Ok(status) => status,
            Err(e) => {
                let err_msg = format!("Failed to delete the target file with id {}. {}", id, e);

                // log
                error!(target: "stdout", "{}", &err_msg);

                DeleteFileStatus {
                    id: id.into(),
                    object: "file".to_string(),
                    deleted: false,
                }
            }
        };

        // serialize status
        let s = match serde_json::to_string(&status) {
            Ok(s) => s,
            Err(e) => {
                let err_msg = format!(
                    "Failed to serialize the status of the file deletion operation. {}",
                    e
                );

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
    } else if req.method() == Method::OPTIONS {
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
                error!(target: "files_handler", "{}", &err_msg);

                return error::internal_server_error(err_msg);
            }
        }
    } else {
        let err_msg = "Invalid HTTP Method.";

        // log
        error!(target: "stdout", "{}", &err_msg);

        error::internal_server_error(err_msg)
    };

    info!(target: "stdout", "Send the files response");

    res
}

fn list_files() -> Response<Body> {
    match llama_core::files::list_files() {
        Ok(file_objects) => {
            // serialize chat completion object
            let s = match serde_json::to_string(&file_objects) {
                Ok(s) => s,
                Err(e) => {
                    let err_msg = format!("Failed to serialize file list. {}", e);

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
        Err(e) => {
            let err_msg = format!("Failed to list all files. {}", e);

            // log
            error!(target: "stdout", "{}", &err_msg);

            error::internal_server_error(err_msg)
        }
    }
}
