use crate::plugin::plugin_manager::PluginError;
use crate::plugin_old::models::{HttpHeader, HttpRequest, HttpResponse};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type CallbackFn = Box<
    dyn Fn(HttpRequest) -> BoxFuture<'static, Result<HttpResponse, PluginError>>
        + Send
        + Sync
        + Unpin,
>;

pub trait Webserver {
    fn set_listener(listener: CallbackFn);
}

pub struct DefaultWebserver {
    listener: Arc<Mutex<Option<CallbackFn>>>,
}

impl Service<Request<Incoming>> for DefaultWebserver {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let listener = self.listener.clone();
        Box::pin(async move {
            let listener = listener.lock().await;
            if let Some(listener) = &*listener {
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

                let response_result = listener(request).await;
                if let Ok(response) = response_result {

                }
            }
            todo!();
        })
    }
}
