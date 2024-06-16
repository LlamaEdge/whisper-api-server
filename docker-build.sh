#!/bin/bash

# Check and download the necessary files if they do not already exist
[ -f tiny_en.tar.gz ] || curl -LO https://huggingface.co/second-state/whisper-burn/resolve/main/tiny_en.tar.gz

# Extract the model file if it exists and hasn't been extracted yet
[ -f tiny_en.mpk ] || tar -xvzf tiny_en.tar.gz

# Build the Docker image with the specified platform
docker build . --platform wasi/wasm -t burn-whisper-server