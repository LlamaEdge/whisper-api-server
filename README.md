# Whisper API Server

## Quick Start

- Download `whisper-burn/tiny_en` model

  ```bash
  curl -LO https://huggingface.co/second-state/whisper-burn/resolve/main/tiny_en.tar.gz
  ```

  Then, unzip the `tiny_en.tar.gz` file to get the `tiny_en.mpk`, `tiny_en.cfg`, and `tokenizer.json` files.

  ```bash
  tar -xvzf tiny_en.tar.gz
  ```

- Download audio file

  ```bash
  curl -LO https://huggingface.co/second-state/whisper-burn/resolve/main/audio16k.wav
  ```

- Download `whisper-api-server.wasm` binary

  ```bash
  curl -LO https://github.com/LlamaEdge/whisper-api-server/raw/main/whisper-api-server.wasm
  ```

- Start `whisper-api-server`

  ```bash
  wasmedge --dir .:. \
    --nn-preload default:Burn:CPU:tiny_en.mpk:tiny_en.cfg:tokenizer.json:en \
    whisper-api-server.wasm
  ```

  > [!NOTE]
  > The `wasmedge-burn` plugin is required to run the `whisper-api-server.wasm` binary. See [Build plugin](https://hackmd.io/@vincent-2nd/SkI3Fh_S0#Build-plugin) to build the plugin from source.
  > For Apple Silicon users, you can download the plugin [here](https://github.com/second-state/wasmedge-burn-plugin/raw/main/libwasmedgePluginWasiNN.dylib).

- Send `curl` request to the transcriptions endpoint

  ```bash
  curl http://localhost:10086/v1/audio/transcriptions \
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

  cargo build --release --target wasm32-wasi
  ```

  If the build is successful, you should see the `whisper-api-server.wasm` binary in the `target/wasm32-wasi/release` directory.

## CLI Options

```bash
$ wasmedge whisper-api-server.wasm -h

Whisper API Server

Usage: whisper-api-server.wasm [OPTIONS]

Options:
  -m, --model-name <MODEL_NAME>    Model name [default: default]
      --model-alias <MODEL_ALIAS>  Model alias [default: default]
      --socket-addr <SOCKET_ADDR>  Socket address of Whisper API server instance [default: 0.0.0.0:8080]
  -h, --help                       Print help
  -V, --version                    Print version
```
