FROM scratch

# Copy the prepared files from the current directory to the image
COPY tiny_en.cfg /tiny_en.cfg
COPY tiny_en.mpk /tiny_en.mpk
COPY tokenizer.json /tokenizer.json
COPY whisper-api-server.wasm /app.wasm

# Set the entrypoint
ENTRYPOINT [ "/app.wasm" ]