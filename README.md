# Whisper API Server

This project is a RESTful API server that provides endpoints for transcribing and translating audio files. The APIs are compitable with OpenAI APIs of [transcriptions and translations](https://platform.openai.com/docs/api-reference/audio).

The following is the list of supported audio formats and codecs:

- The formats supported are `caf`, `isomp4`, `mkv`, `ogg`, `aiff`, `wav`.

- The codecs supported are `aac`, `adpcm`, `alac`, `flac`, `mp1`, `mp2`, `mp3`, `pcm`, `vorbis`.

> [!NOTE]
> The project is still under active development. The existing features still need to be improved and more features will be added in the future.

## Quick Start

### Setup

- Install `WasmEdge v0.14.1` with `wasi_nn-whisper` plugin

  ```bash
  curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install_v2.sh | bash -s -- -v 0.14.1
  ```

- Deploy `wasi_nn-whisper` plugin

  ```bash
  # Download whisper plugin for Mac Apple Silicon
  curl -LO https://github.com/WasmEdge/WasmEdge/releases/download/0.14.1/WasmEdge-plugin-wasi_nn-whisper-0.14.1-darwin_arm64.tar.gz

  # Unzip the plugin to $HOME/.wasmedge/plugin
  tar -xzf WasmEdge-plugin-wasi_nn-whisper-0.14.1-darwin_arm64.tar.gz -C $HOME/.wasmedge/plugin
  ```

### Run whisper-api-server

- Download `whisper-api-server.wasm` binary

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/releases/download/0.3.0/whisper-api-server.wasm
  ```

- Download model

  `ggml` whisper models are available from [https://huggingface.co/ggerganov/whisper.cpp/tree/main](https://huggingface.co/ggerganov/whisper.cpp/tree/main)

  In the following command, `ggml-medium.bin` is downloaded as an example. You can replace it with other models.

  ```bash
  curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin
  ```

- Start server

  ```bash
  wasmedge --dir .:. whisper-api-server.wasm -m ggml-medium.bin
  ```

  To start the server on other port, use `--socket-addr` to specify the port you want to use, for example:

  ```bash
  wasmedge --dir .:. whisper-api-server.wasm -m ggml-medium.bin --socket-addr 0.0.0.0:10086
  ```

  To start the server with api-key, set the environment variable `API_KEY` to specify the api-key, for example:

  ```bash
  wasmedge --dir .:. --env API_KEY=your_api_key whisper-api-server.wasm -m ggml-medium.bin
  ```

### Usage

#### Transcribe an audio file

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

#### Translate an audio file

- Download audio file

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/raw/main/data/test_cn.wav
  ```

  This audio contains a Chinese sentence, `这里是中文广播`, the English meaning is `This is a Chinese broadcast`.

- Send `curl` request to the translations endpoint

  ```bash
  curl --location 'http://localhost:8080/v1/audio/translations' \
    --header 'Content-Type: multipart/form-data' \
    --form 'file=@"test.wav"' \
    --form 'language="zh"'
  ```

  **Note** that the correct value of `language` can be found in [ISO-639-1](https://en.wikipedia.org/wiki/List_of_ISO_639_language_codes)

  If everything is set up correctly, you should see the following generated transcriptions:

  ```json
  {
    "text": "[00:00:00.000 --> 00:00:04.000]  This is a Chinese broadcast."
  }
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
      --threads <THREADS>          Number of threads to use during computation [default: 4]
      --processors <PROCESSORS>    Number of processors to use during computation [default: 1]
      --task <TASK>                Task type [default: full] [possible values: transcribe, translate, full]
      --no-audio-preprocessor      Do not pre-process input audio files
      --port <PORT>                Port number [default: 8080]
      --socket-addr <SOCKET_ADDR>  Socket address of LlamaEdge API Server instance. For example, `0.0.0.0:8080`
  -h, --help                       Print help (see more with '--help')
  -V, --version                    Print version
```
