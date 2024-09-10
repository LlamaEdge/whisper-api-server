
# test /v1/audio/translations endpoint
POST http://localhost:8080/v1/audio/translations
[MultipartFormData]
file: file,test.wav;
HTTP 200