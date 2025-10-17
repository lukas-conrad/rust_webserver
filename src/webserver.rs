use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use log::{error, info};
use std::convert::Infallible;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose};

use crate::plugin::models::{HttpHeader, HttpRequest};
use crate::plugin::{Plugin, PluginManager};

pub(crate) struct WebServer {
    plugin_manager: Arc<PluginManager>,
}

impl WebServer {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            plugin_manager: plugin_manager.clone(),
        }
    }

    async fn find_matching_plugin(
        &self,
        method: &str,
        host: &str,
        path: &str,
    ) -> Option<Arc<Plugin>> {
        let plugins = self.plugin_manager.get_active_plugins().await;

        let mut best_match: Option<(usize, Arc<Plugin>)> = None;

        for plugin in plugins {
            let mut specificity = 0;
            let mut matches = false;

            for req_method in &plugin.config.request_information.request_methods {
                if req_method == "*" || req_method == method {
                    matches = true;
                    if req_method != "*" {
                        specificity += 1;
                    }
                    break;
                }
            }

            if !matches {
                continue;
            }

            matches = false;
            for host_pattern in &plugin.config.request_information.hosts {
                if self.match_pattern(host_pattern, host) {
                    matches = true;
                    if host_pattern != "*" {
                        // Count the number of fixed segments for specificity
                        specificity += host_pattern.chars().filter(|&c| c != '*').count();
                    }
                    break;
                }
            }

            if !matches {
                continue;
            }

            matches = false;
            for path_pattern in &plugin.config.request_information.paths {
                if self.match_pattern(path_pattern, path) {
                    matches = true;
                    if path_pattern != "*" {
                        // Count the number of fixed segments for specificity
                        specificity += path_pattern.chars().filter(|&c| c != '*').count();
                    }
                    break;
                }
            }

            if !matches {
                continue;
            }

            // Update best match if this plugin is more specific
            if let Some((best_specificity, _)) = best_match {
                if specificity > best_specificity {
                    best_match = Some((specificity, plugin));
                }
            } else {
                best_match = Some((specificity, plugin));
            }
        }

        best_match.map(|(_, plugin)| plugin)
    }

    fn match_pattern(&self, pattern: &str, value: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        let regex_pattern = pattern
            .replace(".", "\\.")
            .replace("**/", "(.+/)?")
            .replace("*", "[^/]*");

        let regex = regex::Regex::new(&format!("^{}$", regex_pattern)).unwrap_or_else(|_| {
            regex::Regex::new("^$").unwrap()
        });

        regex.is_match(value)
    }

    async fn process_request(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        let method = req.method().as_str().to_string();
        let path = req.uri().path().to_string();

        let host = req
            .headers()
            .get("host")
            .map(|h| h.to_str().unwrap_or(""))
            .unwrap_or("")
            .to_string();

        let headers: Vec<HttpHeader> = req
            .headers()
            .iter()
            .map(|(name, value)| HttpHeader {
                key: name.to_string(),
                value: value.to_str().unwrap_or("").to_string(),
            })
            .collect();

        let maybe_plugin = self.find_matching_plugin(&method, &host, &path).await;

        match maybe_plugin {
            Some(plugin) => {
                info!("Routing request to plugin: {}", plugin.config.plugin_name);

                let (_, body) = req.into_parts();
                let body_bytes = body.collect().await.unwrap_or_default().to_bytes();
                let body_str = String::from_utf8_lossy(&body_bytes).to_string();

                let http_request = HttpRequest {
                    request_method: method,
                    path,
                    host,
                    headers,
                    body: body_str,
                };

                let plugin = match self
                    .plugin_manager
                    .get_plugin(&plugin.config.plugin_name)
                    .await
                {
                    Some(plugin) => plugin,
                    None => {
                        error!("Failed to get plugin: {}", plugin.config.plugin_name);
                        return Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Full::new(Bytes::from("Plugin not available")))
                            .unwrap();
                    }
                };

                match plugin.handle_request(http_request).await {
                    Ok(plugin_response) => {
                        // Decode the base64-encoded body
                        let body_bytes = match general_purpose::STANDARD.decode(&plugin_response.body) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                error!("Failed to decode base64 body: {}", e);
                                return Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Full::new(Bytes::from("Failed to decode response body")))
                                    .unwrap();
                            }
                        };

                        // Convert the plugin response to an HTTP response
                        let mut response_builder = Response::builder().status(StatusCode::OK);

                        // Add headers
                        for header in plugin_response.headers {
                            response_builder = response_builder.header(header.key, header.value);
                        }

                        response_builder
                            .body(Full::new(Bytes::from(body_bytes)))
                            .unwrap_or_else(|_| {
                                Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Full::new(Bytes::from("Error building response")))
                                    .unwrap()
                            })
                    }
                    Err(err) => {
                        error!("Plugin request failed: {}", err);
                        Response::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .body(Full::new(Bytes::from(format!("Plugin error: {}", err))))
                            .unwrap()
                    }
                }
            }
            None => {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Full::new(Bytes::from(
                        "No plugin found to handle this request",
                    )))
                    .unwrap()
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct WebServerService {
    pub(crate) server: Arc<WebServer>,
}

impl Service<Request<Incoming>> for WebServerService {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let server = self.server.clone();
        Box::pin(async move {
            let response = server.process_request(req).await;
            Ok(response)
        })
    }
}
