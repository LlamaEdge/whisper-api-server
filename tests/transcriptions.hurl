# test /v1/audio/transcriptions endpoint
POST http://localhost:8080/v1/audio/transcriptions
[MultipartFormData]
file: file,test.wav;
language: en;
HTTP 200
[Asserts]
jsonpath "$.text" contains "This is a test record"
