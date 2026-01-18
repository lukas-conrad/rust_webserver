use crate::plugin::plugin_manager::PluginError;
use crate::plugin_old::models::{HttpHeader, HttpRequest, HttpResponse};
use crate::webserver_old::WebServerService;
use base64::engine::general_purpose;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use strum::Display;
use tokio::net::TcpListener;
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

pub struct Http1Server {
    listener: Arc<Mutex<Option<CallbackFn>>>,
}

impl Http1Server {
    async fn start(addr: SocketAddr) -> Result<Arc<Self>, std::io::Error> {
        let server = Arc::new(Self {
            listener: Arc::new(Mutex::new(None)),
        });

        let listener = TcpListener::bind(addr).await?;

        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let service = server.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                error!("Connection error: {:?}", err);
            }
        });

        Ok(server)
    }

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

impl Webserver for Http1Server {
    fn set_listener(&self, listener: CallbackFn) {
        let web_listener = self.listener.clone();
        tokio::spawn(async move {
            let _ = web_listener.lock().await.insert(listener);
        });
    }
}

impl Service<Request<Incoming>> for Http1Server {
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

#[cfg(test)]
mod tests {
    use crate::plugin_old::models::{HttpRequest, HttpResponse};
    use crate::webserver::webserver::{Http1Server, Webserver};
    use futures::FutureExt;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use bytes::Bytes;
    use hyper::Request;
    use tokio::sync::{oneshot, Mutex};

    #[tokio::test]
    async fn test_webserver() {
        let server: Arc<dyn Webserver> = Http1Server::start(SocketAddr::from(([0, 0, 0, 0], 60724)))
            .await
            .unwrap();

        let (sender, receiver) = oneshot::channel::<HttpRequest>();
        let sender = Arc::new(Mutex::new(Some(sender)));
        server.set_listener(Box::new(move |request| {
            let sender = sender.clone();
            async move {
                sender.lock().await.take().unwrap().send(request).unwrap();
                Ok(HttpResponse {
                    headers: vec![],
                    status_code: 200,
                    body: "".to_string(),
                })
            }
            .boxed()
        }));


        // TCP-Verbindung zum Server aufbauen
        let stream = tokio::net::TcpStream::connect("127.0.0.1:8080")
            .await
            .unwrap();
        let io = hyper_util::rt::TokioIo::new(stream);

        // HTTP-Client-Verbindung erstellen
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();

        // Connection in eigenem Task laufen lassen
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                eprintln!("Connection failed: {:?}", err);
            }
        });

        // Request senden
        let req = Request::builder()
            .uri("/test")
            .method("GET")
            .body(http_body_util::Empty::<Bytes>::new())
            .unwrap();

        let _res = sender.send_request(req).await.unwrap();

        let request = receiver.await.unwrap();

        assert_eq!(request.body, "");

    }
}
