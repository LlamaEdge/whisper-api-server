
# test /v1/audio/translations endpoint
POST http://localhost:12345/v1/audio/translations
[MultipartFormData]
file: file,test_cn.wav;
language: cn;
HTTP 200
[Asserts]
jsonpath "$.text" contains "This is a Chinese broadcast"