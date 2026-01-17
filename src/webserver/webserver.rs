use std::convert::Infallible;
use std::sync::Arc;
use bytes::Bytes;
use crate::plugin_old::models::{HttpHeader, HttpRequest, HttpResponse};
use futures::future::BoxFuture;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper::service::Service;
use tokio::sync::Mutex;

pub type CallbackFn =
    Box<dyn Fn(HttpRequest) -> BoxFuture<'static, HttpResponse> + Send + Sync + Unpin>;

pub trait Webserver {

    fn set_listener(listener: CallbackFn);

}

pub struct DefaultWebserver{
    listener: Arc<Mutex<Option<CallbackFn>>>
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

            }
            todo!();
        })
    }
}