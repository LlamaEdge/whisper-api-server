# audio-api-server

```bash
curl http://localhost:10086/v1/audio/transcriptions \
  -H "Content-Type: multipart/form-data" \
  -F file="@/path/to/audio/file/test-123.m4a" \
  -F model="audio2text"
```
