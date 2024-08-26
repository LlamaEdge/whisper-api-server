# Whisper API Server

## Quick Start

- Install `WasmEdge v0.14.1` with `wasi_nn-whisper` plugin

  TODO: Add the installation guide

- Download audio file

  ```bash
  curl -LO https://huggingface.co/second-state/whisper-burn/resolve/main/audio16k.wav
  ```

- Download whisper model file

  ```bash
  curl -sSf https://raw.githubusercontent.com/ggerganov/whisper.cpp/master/models/download-ggml-model.sh | bash -s -- base.en
  ```

  The model will be store at `./ggml-base.en.bin`

- Download `whisper-api-server.wasm` binary

  ```bash
  # specify the version of whisper-api-server
  export version=0.2.0

  # download the whisper-api-server.wasm binary
  curl -LO https://github.com/LlamaEdge/whisper-api-server/releases/download/$version/whisper-api-server.wasm
  ```

- Start `whisper-api-server`

  ```bash
  wasmedge --dir .:. \
    whisper-api-server.wasm \
    --model ggml-base.en.bin
  ```

- Send `curl` request to the transcriptions endpoint

  ```bash
  curl http://localhost:8080/v1/audio/transcriptions \
    -H "Content-Type: multipart/form-data" \
    -F file="@audio16k.wav"
  ```

  If everything is set up correctly, you should see the transcriptions result:

  ```json
  {
      "text": " Hello, I am the whisper machine learning model. If you see this as text then I am working properly."
  }
  ```

## Build

- Clone the repository

  ```bash
  git clone https://github.com/LlamaEdge/whisper-api-server.git
  ```

- Build the `whisper-api-server.wasm` binary

  ```bash
  cd whisper-api-server

  cargo build --release
  ```

  If the build is successful, you should see the `whisper-api-server.wasm` binary in the `target/wasm32-wasip1/release` directory.

## CLI Options

```bash
$ wasmedge whisper-api-server.wasm -h

Whisper API Server

Usage: whisper-api-server.wasm [OPTIONS] --model <MODEL>

Options:
  -n, --model-name <MODEL_NAME>    Model name [default: default]
  -a, --model-alias <MODEL_ALIAS>  Model alias [default: default]
  -m, --model <MODEL>              Path to the whisper model file
      --socket-addr <SOCKET_ADDR>  Socket address of Whisper API server instance [default: 0.0.0.0:8080]
  -h, --help                       Print help
  -V, --version                    Print version
```
