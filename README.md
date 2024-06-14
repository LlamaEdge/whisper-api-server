# Whisper API Server

## Quick Start

- Download `whisper-burn/tiny_en` model

  ```bash
  curl -LO https://huggingface.co/Gadersd/whisper-burn/raw/main/tiny_en/tiny_en.cfg
  curl -LO https://huggingface.co/Gadersd/whisper-burn/resolve/main/tiny_en/tiny_en.mpk.gz
  curl -LO https://huggingface.co/Gadersd/whisper-burn/raw/main/tiny_en/tokenizer.json
  ```

- Unzip `tiny_en.mpk.gz`

  ```bash
  gunzip tiny_en.mpk.gz
  ```

- Download audio file

  ```bash
  curl -LO https://github.com/LlamaEdge/audio-api-server/raw/main/data/audio16k.wav
  ```

- Download `whisper-api-server.wasm` binary

  ```bash
  curl -LO https://github.com/LlamaEdge/audio-api-server/raw/main/whisper-api-server.wasm
  ```

- Start `whisper-api-server`

  ```bash
  wasmedge --dir .:. \
    --nn-preload default:Burn:CPU:tiny_en.mpk:tiny_en.cfg:tokenizer.json:en \
    audio-api-server.wasm
  ```

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
