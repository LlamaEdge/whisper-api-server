use crate::{
    error::{self, LlamaCoreError},
    GRAPH, MAX_BUFFER_SIZE,
};
use endpoints::{
    audio::{TranscriptionObject, TranscriptionRequest},
    files::FileObject,
};
use hound::{self, SampleFormat};
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

                        if !(filename).to_lowercase().ends_with(".wav") {
                            let err_msg = "The audio file (*.wav) must be have a sample rate of 16k and be single-channel.";

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

                                if let Err(e) = field.data.read_to_string(&mut model) {
                                    let err_msg = format!("Failed to read the model. {}", e);

                                    // log
                                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                                    return error::internal_server_error(err_msg);
                                }

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

            let path = Path::new("archives")
                .join(&request.file.id)
                .join(&request.file.filename);

            // load the audio waveform
            let (waveform, sample_rate) = match load_audio_waveform(path) {
                Ok((w, sr)) => (w, sr),
                Err(e) => {
                    let err_msg = format!("Failed to load audio file. {}", e);

                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };
            assert_eq!(sample_rate, 16000, "The audio sample rate must be 16k.");

            info!(target: "audio_transcriptions_handler", "Get the model instance.");
            let graph = match GRAPH.get() {
                Some(graph) => graph,
                None => {
                    let err_msg = "The GRAPH is not initialized.";

                    error!(target: "audio_transcriptions_handler", "{}", err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let mut graph = match graph.lock() {
                Ok(graph) => graph,
                Err(e) => {
                    let err_msg = format!("Failed to lock the graph. {}", e);

                    error!(target: "audio_transcriptions_handler", "{}", err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            // set the input tensor
            info!(target: "audio_transcriptions_handler", "Feed the audio data to the model.");
            if graph
                .set_input(
                    0,
                    wasmedge_wasi_nn::TensorType::F32,
                    &[1, waveform.len()],
                    &waveform,
                )
                .is_err()
            {
                let err_msg = "Fail to set input tensor.";

                error!(target: "audio_transcriptions_handler", "{}", err_msg);

                return error::internal_server_error(err_msg);
            };

            // compute the graph
            info!(target: "audio_transcriptions_handler", "Transcribe audio to text.");
            if let Err(e) = graph.compute() {
                let err_msg = format!("Fail to compute the graph. {}", e);

                error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                return error::internal_server_error(err_msg);
            }

            // get the output tensor
            info!(target: "audio_transcriptions_handler", "Retrieve the transcription data.");
            let mut output_buffer = vec![0u8; MAX_BUFFER_SIZE];
            match graph.get_output(0, &mut output_buffer) {
                Ok(size) => {
                    unsafe {
                        output_buffer.set_len(size);
                    }
                    info!(target: "audio_transcriptions_handler", "Output buffer size: {}", size);
                }
                Err(e) => {
                    let err_msg = format!("Failed to get the generated output tensor. {}", e);

                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            // decode the output buffer
            info!(target: "audio_transcriptions_handler", "Decode the transcription data to plain text.");
            let text = match std::str::from_utf8(&output_buffer[..]) {
                Ok(output) => output.to_string(),
                Err(e) => {
                    let err_msg = format!(
                        "Failed to decode the gerated buffer to a utf-8 string. {}",
                        e
                    );

                    error!(target: "audio_transcriptions_handler", "{}", &err_msg);

                    return error::internal_server_error(err_msg);
                }
            };

            let obj = TranscriptionObject { text };

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

fn load_audio_waveform(
    filename: impl AsRef<std::path::Path>,
) -> Result<(Vec<f32>, usize), LlamaCoreError> {
    let reader =
        hound::WavReader::open(filename).map_err(|e| LlamaCoreError::Operation(e.to_string()))?;
    let spec = reader.spec();

    // let duration = reader.duration() as usize;
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate as usize;
    // let bits_per_sample = spec.bits_per_sample;
    let sample_format = spec.sample_format;

    assert_eq!(sample_rate, 16000, "The audio sample rate must be 16k.");
    assert_eq!(channels, 1, "The audio must be single-channel.");

    let max_int_val = 2_u32.pow(spec.bits_per_sample as u32 - 1) - 1;

    let floats = match sample_format {
        SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<hound::Result<_>>()
            .map_err(|e| LlamaCoreError::Operation(e.to_string()))?,
        SampleFormat::Int => reader
            .into_samples::<i32>()
            .map(|s| s.map(|s| s as f32 / max_int_val as f32))
            .collect::<hound::Result<_>>()
            .map_err(|e| LlamaCoreError::Operation(e.to_string()))?,
    };

    Ok((floats, sample_rate))
}
