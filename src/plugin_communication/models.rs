use crate::plugin_communication::plugin_communicator::Filter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::{Display, EnumString};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RequestInformation {
    pub request_methods: Vec<String>,
    pub hosts: Vec<String>,
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequest {
    pub request_method: String,
    pub path: String,
    pub host: String,
    pub headers: Vec<HttpHeader>,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponse {
    pub headers: Vec<HttpHeader>,
    pub status_code: u16,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HttpHeader {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLog {
    pub plugin_name: String,
    pub error_type: String,
    pub error_name: String,
    pub error_details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageGen<T> {
    package_type: PackageType,
    pub content: T,
}

impl<T> PackageGen<T> {
    pub fn package_type(&self) -> &PackageType {
        &self.package_type
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeRequestContent {
    pub protocol: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeResponseContent {
    pub response_code: u32,
    pub response_code_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NormalRequestContent {
    pub package_id: i64,
    pub http_request: HttpRequest,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NormalResponseContent {
    pub package_id: i64,
    pub http_response: HttpResponse,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReportContent {
    pub error_code: u32,
    pub error_description: String,
    pub policy: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogContent {
    pub level: String,
    pub message: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownContent {
}

macro_rules! package {
    (
        $(
            $variant:ident($content_type:ty)
        ),* $(,)?
    ) => {
        // Generate PackageType enum
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        pub enum PackageType {
            $(
                $variant,
            )*
        }

        // Generate Package enum
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(tag = "packageType", content = "content", rename_all = "camelCase")]
        #[allow(unused)]
        pub enum Package {
            $(
                $variant($content_type),
            )*
        }

        // Generate type aliases for Package<T>
        $(

            paste::paste! {
                #[allow(unused)]
                pub type [<Package $variant>] = PackageGen<$content_type>;
            }
        )*

        // Implement methods to convert Package<T> to Package
        impl Package {

            #[allow(unused)]
            pub fn package_type(&self) -> PackageType {
                match self {
                    $(
                        Package::$variant(_) => PackageType::$variant,
                    )*
                }
            }
        }

        // Implement From<Package<T>> for Package for each type
        // $(
        //     impl From<Package<$content_type>> for Package {
        //         fn from(package: Package<$content_type>) -> Self {
        //             package.content
        //         }
        //     }
        // )*

        // Implement methods to get Package variant from Package<T>
        $(
            impl PackageGen<$content_type> {

                #[allow(unused)]
                pub fn new(content: $content_type) -> Self {
                    Self {
                        package_type: PackageType::$variant,
                        content
                    }
                }

                #[allow(unused)]
                pub fn filter() -> Filter {
                    Box::new(|package: &Package| {
                        match package {
                            Package::$variant(_) => true,
                            _ => false
                        }
                    })
                }

                #[allow(unused)]
                pub fn to_package(self) -> Package {
                    Package::$variant(self.content)
                }
            }
        )*
    };
}

// Generate PackageType, Package, and type aliases using macro
package! {
    HandshakeRequest(HandshakeRequestContent),
    HandshakeResponse(HandshakeResponseContent),
    NormalRequest(NormalRequestContent),
    NormalResponse(NormalResponseContent),
    Error(ErrorReportContent),
    Log(LogContent),
    ShutdownRequest(ShutdownContent),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::plugin_config::{PluginConfig, ProtocolEnum};
    use serde_json;
    use std::collections::HashMap;

    #[test]
    fn test_handshake_request_serialization() {
        let content = HandshakeRequestContent {
            protocol: "json".to_string(),
        };
        let package = PackageGen {
            package_type: PackageType::HandshakeRequest,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();
        let expected = r#"{"packageType":"handshakeRequest","content":{"protocol":"json"}}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_handshake_request_deserialization() {
        let json = r#"{"packageType":"handshakeRequest","content":{"protocol":"json"}}"#;

        let package: PackageHandshakeRequest = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::HandshakeRequest);
        assert_eq!(package.content.protocol, "json");
    }

    #[test]
    fn test_handshake_response_serialization() {
        let content = HandshakeResponseContent {
            response_code: 0,
            response_code_text: "Success".to_string(),
        };
        let package = PackageGen {
            package_type: PackageType::HandshakeResponse,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();
        let expected = r#"{"packageType":"handshakeResponse","content":{"responseCode":0,"responseCodeText":"Success"}}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_handshake_response_deserialization() {
        let json = r#"{"packageType":"handshakeResponse","content":{"responseCode":1,"responseCodeText":"Plugin initialization error"}}"#;

        let package: PackageHandshakeResponse = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::HandshakeResponse);
        assert_eq!(package.content.response_code, 1);
        assert_eq!(
            package.content.response_code_text,
            "Plugin initialization error"
        );
    }

    #[test]
    fn test_normal_request_serialization() {
        let headers = vec![HttpHeader {
            key: "Accept".to_string(),
            value: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
        }];

        let http_request = HttpRequest {
            request_method: "GET".to_string(),
            path: "home/helloWorld.html".to_string(),
            host: "api.server.de".to_string(),
            headers,
            body: "request body".to_string(),
        };

        let content = NormalRequestContent {
            package_id: 12345,
            http_request,
        };

        let package = PackageGen {
            package_type: PackageType::NormalRequest,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();

        // Check that it contains expected fields
        assert!(json.contains(r#""packageType":"normalRequest""#));
        assert!(json.contains(r#""packageId":12345"#));
        assert!(json.contains(r#""requestMethod":"GET""#));
        assert!(json.contains(r#""path":"home/helloWorld.html""#));
        assert!(json.contains(r#""host":"api.server.de""#));
    }

    #[test]
    fn test_normal_request_deserialization() {
        let json = r#"{
            "packageType": "normalRequest",
            "content": {
                "packageId": 12345,
                "httpRequest": {
                    "requestMethod": "GET",
                    "path": "home/helloWorld.html",
                    "host": "api.server.de",
                    "headers": [
                        {
                            "key": "Accept",
                            "value": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                        }
                    ],
                    "body": "request body"
                }
            }
        }"#;

        let package: PackageNormalRequest = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::NormalRequest);
        assert_eq!(package.content.package_id, 12345);
        assert_eq!(package.content.http_request.request_method, "GET");
        assert_eq!(package.content.http_request.path, "home/helloWorld.html");
        assert_eq!(package.content.http_request.host, "api.server.de");
        assert_eq!(package.content.http_request.headers.len(), 1);
        assert_eq!(package.content.http_request.headers[0].key, "Accept");
        assert_eq!(package.content.http_request.body, "request body");
    }

    #[test]
    fn test_normal_response_serialization() {
        let headers = vec![HttpHeader {
            key: "Content-Encoding".to_string(),
            value: "gzip".to_string(),
        }];

        let http_response = HttpResponse {
            headers,
            status_code: 200,
            body: "response body".to_string(),
        };

        let content = NormalResponseContent {
            package_id: 12345,
            http_response,
        };

        let package = PackageGen {
            package_type: PackageType::NormalResponse,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();

        assert!(json.contains(r#""packageType":"normalResponse""#));
        assert!(json.contains(r#""packageId":12345"#));
        assert!(json.contains(r#""statusCode":200"#));
    }

    #[test]
    fn test_normal_response_deserialization() {
        let json = r#"{
            "packageType": "normalResponse",
            "content": {
                "packageId": 12345,
                "httpResponse": {
                    "headers": [
                        {
                            "key": "Content-Encoding",
                            "value": "gzip"
                        }
                    ],
                    "statusCode": 200,
                    "body": "response body"
                }
            }
        }"#;

        let package: PackageNormalResponse = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::NormalResponse);
        assert_eq!(package.content.package_id, 12345);
        assert_eq!(package.content.http_response.status_code, 200);
        assert_eq!(package.content.http_response.body, "response body");
        assert_eq!(package.content.http_response.headers.len(), 1);
        assert_eq!(
            package.content.http_response.headers[0].key,
            "Content-Encoding"
        );
        assert_eq!(package.content.http_response.headers[0].value, "gzip");
    }

    #[test]
    fn test_error_report_serialization() {
        let content = ErrorReportContent {
            error_code: 15902,
            error_description: "Fatal error, plugin_old is corrupt".to_string(),
            policy: "restart".to_string(),
        };

        let package = PackageGen {
            package_type: PackageType::Error,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();
        let expected = r#"{"packageType":"error","content":{"errorCode":15902,"errorDescription":"Fatal error, plugin_old is corrupt","policy":"restart"}}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_error_report_deserialization() {
        let json = r#"{
            "packageType": "error",
            "content": {
                "errorCode": 15902,
                "errorDescription": "Fatal error, plugin_old is corrupt",
                "policy": "restart"
            }
        }"#;

        let package: PackageError = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::Error);
        assert_eq!(package.content.error_code, 15902);
        assert_eq!(
            package.content.error_description,
            "Fatal error, plugin_old is corrupt"
        );
        assert_eq!(package.content.policy, "restart");
    }

    #[test]
    fn test_log_message_serialization() {
        let content = LogContent {
            level: "info".to_string(),
            message: "Successfully processed request".to_string(),
        };

        let package = PackageGen {
            package_type: PackageType::Log,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();
        let expected = r#"{"packageType":"log","content":{"level":"info","message":"Successfully processed request"}}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_log_message_deserialization() {
        let json = r#"{
            "packageType": "log",
            "content": {
                "level": "warning",
                "message": "Something unexpected happened"
            }
        }"#;

        let package: PackageLog = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::Log);
        assert_eq!(package.content.level, "warning");
        assert_eq!(package.content.message, "Something unexpected happened");
    }

    #[test]
    fn test_shutdown_request_serialization() {
        let content: HashMap<String, String> = HashMap::new();

        let package = PackageGen {
            package_type: PackageType::ShutdownRequest,
            content,
        };

        let json = serde_json::to_string(&package).unwrap();
        let expected = r#"{"packageType":"shutdownRequest","content":{}}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_invalid_json_deserialization() {
        let invalid_json = r#"{"packageType": "startup", "invalid": "structure"}"#;

        let result: Result<PackageHandshakeRequest, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_fields_deserialization() {
        let incomplete_json = r#"{"packageType": "startup"}"#;

        let result: Result<PackageHandshakeRequest, _> = serde_json::from_str(incomplete_json);
        assert!(result.is_err());
    }
    #[test]
    fn test_shutdown_request_deserialization() {
        let json = r#"{
            "packageType": "shutdownRequest",
            "content": {}
        }"#;

        let package: PackageShutdownRequest = serde_json::from_str(json).unwrap();

        assert_eq!(package.package_type, PackageType::ShutdownRequest);
        assert!(package.content == ShutdownContent{});
    }

    #[test]
    fn test_generic_package_startup_deserialization() {
        let json = r#"{
            "packageType": "handshakeRequest",
            "content": {
                "protocol": "json"
            }
        }"#;

        let package: Package = serde_json::from_str(json).unwrap();

        assert_eq!(matches!(package, Package::HandshakeRequest(_)), true);
        match package {
            Package::HandshakeRequest(content) => {
                assert_eq!(content.protocol, "json");
            }
            _ => panic!("Expected Startup content"),
        }
    }

    #[test]
    fn test_generic_package_startup_response_deserialization() {
        let json = r#"{
            "packageType": "handshakeResponse",
            "content": {
                "responseCode": 0,
                "responseCodeText": "Success"
            }
        }"#;

        let package: Package = serde_json::from_str(json).unwrap();

        assert!(matches!(package, Package::HandshakeResponse(_)));
        match package {
            Package::HandshakeResponse(content) => {
                assert_eq!(content.response_code, 0);
                assert_eq!(content.response_code_text, "Success");
            }
            _ => panic!("Expected StartupResponse content"),
        }
    }

    #[test]
    fn test_generic_package_unknown_deserialization() {
        let json = r#"{
            "packageType": "unknown",
            "content": {
                "someField": "someValue"
            }
        }"#;

        let package: serde_json::error::Result<Package> = serde_json::from_str(json);

        assert!(matches!(package, Err(_)));
    }

    #[test]
    fn test_package_type_serialization() {
        assert_eq!(
            serde_json::to_string(&PackageType::HandshakeRequest).unwrap(),
            r#""handshakeRequest""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::HandshakeResponse).unwrap(),
            r#""handshakeResponse""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::NormalRequest).unwrap(),
            r#""normalRequest""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::NormalResponse).unwrap(),
            r#""normalResponse""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::ShutdownRequest).unwrap(),
            r#""shutdownRequest""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::Error).unwrap(),
            r#""error""#
        );
        assert_eq!(
            serde_json::to_string(&PackageType::Log).unwrap(),
            r#""log""#
        );
    }

    #[test]
    fn test_package_type_deserialization() {
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""handshakeRequest""#).unwrap(),
            PackageType::HandshakeRequest
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""handshakeResponse""#).unwrap(),
            PackageType::HandshakeResponse
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""normalRequest""#).unwrap(),
            PackageType::NormalRequest
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""normalResponse""#).unwrap(),
            PackageType::NormalResponse
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""shutdownRequest""#).unwrap(),
            PackageType::ShutdownRequest
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""error""#).unwrap(),
            PackageType::Error
        );
        assert_eq!(
            serde_json::from_str::<PackageType>(r#""log""#).unwrap(),
            PackageType::Log
        );
    }

    #[test]
    fn test_error_log_serialization() {
        let error_log = ErrorLog {
            plugin_name: "Internal Business logic".to_string(),
            error_type: "ValidationError".to_string(),
            error_name: "Invalid response from Plugin".to_string(),
            error_details: "Plugin returned a invalid json: { packageN87q24ijo }".to_string(),
        };

        let json = serde_json::to_string(&error_log).unwrap();

        assert!(json.contains(r#""pluginName":"Internal Business logic""#));
        assert!(json.contains(r#""errorType":"ValidationError""#));
        assert!(json.contains(r#""errorName":"Invalid response from Plugin""#));
        assert!(json
            .contains(r#""errorDetails":"Plugin returned a invalid json: { packageN87q24ijo }""#));
    }

    #[test]
    fn test_error_log_deserialization() {
        let json = r#"{
            "pluginName": "Internal Business logic",
            "errorType": "PluginError",
            "errorName": "Plugin returned error code",
            "errorDetails": "Error code 5 with message: Database connection failed"
        }"#;

        let error_log: ErrorLog = serde_json::from_str(json).unwrap();

        assert_eq!(error_log.plugin_name, "Internal Business logic");
        assert_eq!(error_log.error_type, "PluginError");
        assert_eq!(error_log.error_name, "Plugin returned error code");
        assert_eq!(
            error_log.error_details,
            "Error code 5 with message: Database connection failed"
        );
    }

    #[test]
    fn test_complex_http_request_with_multiple_headers() {
        let headers = vec![
            HttpHeader {
                key: "Accept".to_string(),
                value: "application/json".to_string(),
            },
            HttpHeader {
                key: "Authorization".to_string(),
                value: "Bearer token123".to_string(),
            },
            HttpHeader {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
        ];

        let http_request = HttpRequest {
            request_method: "POST".to_string(),
            path: "api/v1/users".to_string(),
            host: "api.example.com".to_string(),
            headers,
            body: r#"{"name": "John Doe", "email": "john@example.com"}"#.to_string(),
        };

        let json = serde_json::to_string(&http_request).unwrap();
        let deserialized: HttpRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.request_method, "POST");
        assert_eq!(deserialized.path, "api/v1/users");
        assert_eq!(deserialized.host, "api.example.com");
        assert_eq!(deserialized.headers.len(), 3);
        assert_eq!(
            deserialized.body,
            r#"{"name": "John Doe", "email": "john@example.com"}"#
        );
    }

    #[test]
    fn test_plugin_config_serialization() {
        let request_info = RequestInformation {
            request_methods: vec!["*".to_string()],
            hosts: vec!["api.server.de".to_string(), "business.server.*".to_string()],
            paths: vec!["api/*".to_string(), "api/**/business/*".to_string()],
        };

        let config = PluginConfig {
            plugin_name: "InternalBusinessHandler420".to_string(),
            startup_command: "java -jar businessHandler.jar".to_string(),
            protocol: ProtocolEnum::StdIoJson,
            max_request_timeout: 1000,
            max_startup_time: 1000,
            request_information: request_info,
        };

        let json = serde_json::to_string(&config).unwrap();

        assert!(json.contains(r#""pluginName":"InternalBusinessHandler420""#));
        assert!(json.contains(r#""startupCommand":"java -jar businessHandler.jar""#));
        assert!(json.contains(r#""protocol":"STD_IO_JSON""#));
        assert!(json.contains(r#""maxRequestTimeout":1000"#));
        assert!(json.contains(r#""maxStartupTime":1000"#));
    }

    #[test]
    fn test_plugin_config_deserialization() {
        let json = r#"{
            "pluginName": "InternalBusinessHandler420",
            "startupCommand": "java -jar businessHandler.jar",
            "protocol": "STD_IO_JSON",
            "maxRequestTimeout": 1000,
            "maxStartupTime": 1000,
            "requestInformation": {
                "requestMethods": ["*"],
                "hosts": ["api.server.de", "business.server.*"],
                "paths": ["api/*", "api/**/business/*"]
            }
        }"#;

        let config: PluginConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.plugin_name, "InternalBusinessHandler420");
        assert_eq!(config.startup_command, "java -jar businessHandler.jar");
        assert_eq!(config.protocol, ProtocolEnum::StdIoJson);
        assert_eq!(config.max_request_timeout, 1000);
        assert_eq!(config.max_startup_time, 1000);
        assert_eq!(config.request_information.request_methods, vec!["*"]);
        assert_eq!(
            config.request_information.hosts,
            vec!["api.server.de", "business.server.*"]
        );
        assert_eq!(
            config.request_information.paths,
            vec!["api/*", "api/**/business/*"]
        );
    }

    #[test]
    fn test_http_response_with_different_status_codes() {
        let test_cases = vec![
            (200, "OK"),
            (404, "Not Found"),
            (500, "Internal Server Error"),
        ];

        for (status_code, description) in test_cases {
            let response = HttpResponse {
                headers: vec![],
                status_code,
                body: format!("Status: {}", description),
            };

            let json = serde_json::to_string(&response).unwrap();
            let deserialized: HttpResponse = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.status_code, status_code);
            assert_eq!(deserialized.body, format!("Status: {}", description));
        }
    }

    #[test]
    fn test_all_log_levels() {
        let log_levels = vec!["debug", "info", "warning", "error", "critical"];

        for level in log_levels {
            let content = LogContent {
                level: level.to_string(),
                message: format!("This is a {} message", level),
            };

            let json = serde_json::to_string(&content).unwrap();
            let deserialized: LogContent = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.level, level);
            assert_eq!(deserialized.message, format!("This is a {} message", level));
        }
    }

    #[test]
    fn test_all_error_policies() {
        let policies = vec!["restart", "stop", "report"];

        for policy in policies {
            let content = ErrorReportContent {
                error_code: 100,
                error_description: "Test error".to_string(),
                policy: policy.to_string(),
            };

            let json = serde_json::to_string(&content).unwrap();
            let deserialized: ErrorReportContent = serde_json::from_str(&json).unwrap();

            assert_eq!(deserialized.policy, policy);
            assert_eq!(deserialized.error_code, 100);
            assert_eq!(deserialized.error_description, "Test error");
        }
    }
}
