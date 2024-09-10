# test /v1/audio/transcriptions endpoint
POST http://localhost:8080/v1/audio/transcriptions
[MultipartFormData]
field1: file,test.wav;
HTTP 200
