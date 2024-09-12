
# test /v1/audio/translations endpoint
POST http://localhost:8080/v1/audio/translations
[MultipartFormData]
file: file,test_cn.wav;
HTTP 200
[Asserts]
jsonpath "$.text" contains "这里是中文广播"