# Whisper API Server

## Quick Start

- Install `WasmEdge v0.14.1.rc-4` with `wasi_nn-whisper` plugin

  TODO: Add the installation guide

- Download `whisper-api-server.wasm` binary

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/raw/feat-audio-speech/whisper-api-server.wasm
  ```

- Download whisper model file

  `ggml` whisper models are available from [https://huggingface.co/ggerganov/whisper.cpp/tree/main](https://huggingface.co/ggerganov/whisper.cpp/tree/main)

  In the following command, `ggml-medium.bin` is downloaded as an example. You can replace it with other models.

  ```bash
  curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin
  ```

- Start `whisper-api-server` on default `8080` port

  ```bash
  wasmedge --dir .:. whisper-api-server.wasm -m ggml-medium.bin
  ```

  To start the server on other port, use `--socket-addr` to specify the port you want to use, for example:

  ```bash
  wasmedge --dir .:. whisper-api-server.wasm -m ggml-medium.bin --socket-addr 0.0.0.0:10086
  ```

### Transcribe an audio file

- Download audio file

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/raw/main/data/test.wav

  ```

- Send `curl` request to the transcriptions endpoint

  ```bash
  curl --location 'http://localhost:8080/v1/audio/transcriptions' \
    --header 'Content-Type: multipart/form-data' \
    --form 'file=@"test.wav"'
  ```

  If everything is set up correctly, you should see the following generated transcriptions:

  ```json
  {
      "text": "[00:00:00.000 --> 00:00:03.540]  This is a test record for Whisper.cpp"
  }
  ```

### Translate an audio file

- Download audio file

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/raw/main/data/test_cn.wav
  ```

- Send `curl` request to the translations endpoint

  ```bash
  curl --location 'http://localhost:8080/v1/audio/translations' \
    --header 'Content-Type: multipart/form-data' \
    --form 'file=@"test.wav"'
  ```

## Build

To build the `whisper-api-server.wasm` binary, you need to have the `Rust` toolchain installed. If you don't have it installed, you can install it by following the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

If you are working on macOS, you need to download the `wasi-sdk` from [https://github.com/WebAssembly/wasi-sdk/releases](https://github.com/WebAssembly/wasi-sdk/releases); and then, set the `WASI_SDK_PATH` environment variable to the path of the `wasi-sdk` directory, and set `CC` environment variable to the `clang` of `wasi-sdk`, for example:

  ```bash
  export WASI_SDK_PATH /path/to/wasi-sdk-22.0
  export CC="${WASI_SDK_PATH}/bin/clang --sysroot=${WASI_SDK_PATH}/share/wasi-sysroot"
  ```

Now, you can build the `whisper-api-server.wasm` binary by following the steps below:

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
