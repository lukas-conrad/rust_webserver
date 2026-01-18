use crate::plugin::plugin_manager::PluginError;
use crate::plugin_old::models::{HttpHeader, HttpRequest, HttpResponse};
use base64::engine::general_purpose;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use log::error;
use std::convert::Infallible;
use std::sync::Arc;
use strum::Display;
use tokio::sync::Mutex;

#[derive(Display, Debug)]
pub enum ServerError {
    RequestProcessingError(StatusCode, String),
}

pub type CallbackFn = Box<
    dyn Fn(HttpRequest) -> BoxFuture<'static, Result<HttpResponse, PluginError>>
        + Send
        + Sync
        + Unpin,
>;

pub trait Webserver {
    fn set_listener(&self, listener: CallbackFn);
}

pub struct DefaultWebserver {
    listener: Arc<Mutex<Option<CallbackFn>>>,
}

impl DefaultWebserver {
    async fn build_http_request(req: Request<Incoming>) -> HttpRequest {
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
        let (_, body) = req.into_parts();
        let body_bytes = body.collect().await.unwrap_or_default().to_bytes();

        let request = HttpRequest {
            request_method: method,
            path,
            host,
            headers,
            body: BASE64_STANDARD.encode(body_bytes),
        };
        request
    }
}

impl Webserver for DefaultWebserver {
    fn set_listener(&self, listener: CallbackFn) {
        let web_listener= self.listener.clone();
        tokio::spawn(async move {
            let _ = web_listener.lock().await.insert(listener);
        });
    }
}

impl Service<Request<Incoming>> for DefaultWebserver {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let listener = self.listener.clone();
        Box::pin(async move {
            let result: Result<Response<Full<Bytes>>, ServerError> = async {
                let listener_guard = listener.lock().await;
                let listener =
                    listener_guard
                        .as_ref()
                        .ok_or(ServerError::RequestProcessingError(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "No listener configured".to_string(),
                        ))?;

                let request = Self::build_http_request(req).await;
                let response = listener(request).await.map_err(|err| {
                    ServerError::RequestProcessingError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        err.to_string(),
                    )
                })?;

                let body_bytes = BASE64_STANDARD.decode(&response.body).map_err(|err| {
                    ServerError::RequestProcessingError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        err.to_string(),
                    )
                })?;

                let status_code = StatusCode::from_u16(response.status_code).map_err(|err| {
                    ServerError::RequestProcessingError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        err.to_string(),
                    )
                })?;

                // Convert the plugin_old response to an HTTP response
                let mut response_builder = Response::builder().status(status_code);

                // Add headers
                for header in response.headers {
                    response_builder = response_builder.header(header.key, header.value);
                }

                let client_response = response_builder
                    .body(Full::new(Bytes::from(body_bytes)))
                    .map_err(|err| {
                        ServerError::RequestProcessingError(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            err.to_string(),
                        )
                    })?;

                Ok(client_response)
            }
            .await;

            match result {
                Ok(response) => Ok(response),
                Err(err) => {
                    if let ServerError::RequestProcessingError(code, msg) = err {
                        Ok(Response::builder()
                            .status(code)
                            .body(Full::new(Bytes::from(msg)))
                            .unwrap())
                    } else {
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Full::new(Bytes::from("Internal Server Error")))
                            .unwrap())
                    }
                }
            }
        })
    }
}
