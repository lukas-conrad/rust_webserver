use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use strum::{Display, EnumString};
// Plugin Konfiguration aus der pluginConfig.json

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PluginConfig {
    pub plugin_name: String,
    pub startup_command: String,
    pub protocols: Vec<String>,
    pub max_request_timeout: u64,
    pub max_startup_time: u64,
    pub request_information: RequestInformation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RequestInformation {
    pub request_methods: Vec<String>,
    pub hosts: Vec<String>,
    pub paths: Vec<String>,
}

// HTTP-Strukturen, die in verschiedenen Paket-Typen verwendet werden
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
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HttpHeader {
    pub key: String,
    pub value: String,
}

// Error-Log für persistentes Error-Logging
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorLog {
    pub plugin_name: String,
    pub error_type: String,
    pub error_name: String,
    pub error_details: String,
}

// Paket-Typen als Enum für Typsicherheit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum PackageType {
    Startup,
    StartupResponse,
    Response,
    Request,
    Shutdown,
    Error,
    Log,
    #[serde(other)]
    Unknown,
}

// Einheitliche Paketstruktur für alle Kommunikationsarten
#[derive(Debug, Clone)]
pub struct Package<T> {
    pub package_type: PackageType,
    pub content: T,
}

// Implementierung für Serialisierung
impl<T: Serialize> Serialize for Package<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Package", 2)?;
        state.serialize_field("packageType", &self.package_type)?;
        state.serialize_field("content", &self.content)?;
        state.end()
    }
}

// Implementierung für Deserialisierung mit generischem Content-Typ
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Package<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PackageVisitor<T> {
            _marker: std::marker::PhantomData<T>,
        }

        impl<'de, T: Deserialize<'de>> Visitor<'de> for PackageVisitor<T> {
            type Value = Package<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Package object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Package<T>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut package_type = None;
                let mut content = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "packageType" => {
                            package_type = Some(map.next_value()?);
                        }
                        "content" => {
                            content = Some(map.next_value()?);
                        }
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let package_type =
                    package_type.ok_or_else(|| de::Error::missing_field("packageType"))?;
                let content = content.ok_or_else(|| de::Error::missing_field("content"))?;

                Ok(Package {
                    package_type,
                    content,
                })
            }
        }

        deserializer.deserialize_map(PackageVisitor {
            _marker: std::marker::PhantomData,
        })
    }
}

// Content-Typen für die verschiedenen Paketarten
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

// Typ-Alias-Definitionen für bessere Lesbarkeit und Abwärtskompatibilität
pub type HandshakeRequest = Package<HandshakeRequestContent>;
pub type HandshakeResponse = Package<HandshakeResponseContent>;
pub type NormalRequest = Package<NormalRequestContent>;
pub type NormalResponse = Package<NormalResponseContent>;
pub type ErrorReport = Package<ErrorReportContent>;
pub type LogMessage = Package<LogContent>;
pub type ShutdownRequest = Package<HashMap<String, String>>;

// PackageContent Enum für die Deserialisierung eines generischen Pakets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageContent {
    Startup(HandshakeRequestContent),
    StartupResponse(HandshakeResponseContent),
    Request(NormalRequestContent),
    Response(NormalResponseContent),
    Error(ErrorReportContent),
    Log(LogContent),
    Shutdown(HashMap<String, String>),
    Unknown(serde_json::Value),
}

// Ein generisches Paket, das verschiedene Content-Typen enthalten kann
#[derive(Debug, Clone, Serialize)]
pub struct GenericPackage {
    pub package_type: PackageType,
    pub content: PackageContent,
}

// Implementierung für die Deserialisierung eines generischen Pakets
impl<'de> Deserialize<'de> for GenericPackage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(rename = "packageType")]
            package_type: PackageType,
            content: serde_json::Value,
        }

        let helper = Helper::deserialize(deserializer)?;

        // Basierend auf dem Paket-Typ den Content entsprechend deserialisieren
        let content = match helper.package_type {
            PackageType::Startup => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Startup(content)
                } else {
                    return Err(de::Error::custom("Invalid startup content"));
                }
            }
            PackageType::StartupResponse => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::StartupResponse(content)
                } else {
                    return Err(de::Error::custom("Invalid startup response content"));
                }
            }
            PackageType::Response => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Response(content)
                } else {
                    return Err(de::Error::custom("Invalid response content"));
                }
            }
            PackageType::Request => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Request(content)
                } else {
                    return Err(de::Error::custom("Invalid request content"));
                }
            }
            PackageType::Shutdown => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Shutdown(content)
                } else {
                    PackageContent::Shutdown(HashMap::new())
                }
            }
            PackageType::Error => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Error(content)
                } else {
                    return Err(de::Error::custom("Invalid error content"));
                }
            }
            PackageType::Log => {
                if let Ok(content) = serde_json::from_value(helper.content.clone()) {
                    PackageContent::Log(content)
                } else {
                    return Err(de::Error::custom("Invalid log content"));
                }
            }
            PackageType::Unknown => PackageContent::Unknown(helper.content),
        };

        Ok(GenericPackage {
            package_type: helper.package_type,
            content,
        })
    }
}
