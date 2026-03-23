use crate::plugin_communication::models::{HttpHeader, HttpRequest};
use crate::webserver::webserver::{CallbackFn, ServerError, Webserver};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use log::{error, info};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use std::path::Path;

// TLS
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::{self, ServerConfig};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::{BufReader, Error as IoError};
use hyper::server::conn::http1;

pub struct Https1Server {
    listener: Arc<Mutex<Option<CallbackFn>>>,
}

impl Https1Server {
    pub async fn start(addr: SocketAddr, cert_path: &str, key_path: &str) -> Result<Arc<Self>, std::io::Error> {
        let server = Arc::new(Self {
            listener: Arc::new(Mutex::new(None)),
        });

        let tls_config = load_tls_config(cert_path, key_path)?;
        let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

        let listener = Arc::new(TcpListener::bind(addr).await?);
        let server_clone = server.clone();
        tokio::task::spawn(async move {
            loop {
                let accept = listener.clone().accept().await;
                let server = server_clone.clone();
                match accept {
                    Ok((stream, _)) => {
                        let acceptor = tls_acceptor.clone();
                        let service = server.clone();
                        tokio::task::spawn(async move {
                            match acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    let io = TokioIo::new(tls_stream);
                                    if let Err(e) = http1::Builder::new()
                                        .serve_connection(io, service)
                                        .await
                                    {
                                        error!("HTTP/1.1 connection error: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("TLS handshake error: {:?}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Accept error: {:?}", e);
                    }
                }
            }
        });

        Ok(server)
    }
}

fn load_tls_config(cert_path: &str, key_path: &str) -> Result<ServerConfig, IoError> {
    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| IoError::new(std::io::ErrorKind::InvalidInput, format!("TLS config error: {e}")))?;
    Ok(config)
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>, IoError> {
    let certfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(certfile);
    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .filter_map(|res| res.ok())
        .collect();
    if certs.is_empty() {
        error!("No certificates found in {path}");
        return Err(IoError::new(std::io::ErrorKind::InvalidInput, "No certificates found"));
    } else {
        info!("{} certificates loaded from {path}", certs.len());
    }
    Ok(certs)
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>, IoError> {
    let keyfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(keyfile);
    // Zuerst PKCS#8 versuchen
    let mut pkcs8_keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .filter_map(|res| res.ok());
    if let Some(key) = pkcs8_keys.next() {
        info!("PKCS#8 private key loaded from {path}");
        return Ok(PrivateKeyDer::Pkcs8(key));
    }
    // Reader zurücksetzen und RSA versuchen
    let keyfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(keyfile);
    let mut rsa_keys = rustls_pemfile::rsa_private_keys(&mut reader)
        .filter_map(|res| res.ok());
    if let Some(key) = rsa_keys.next() {
        info!("RSA private key loaded from {path}");
        return Ok(PrivateKeyDer::Pkcs1(key));
    }
    // Reader zurücksetzen und EC versuchen
    let keyfile = File::open(Path::new(path))?;
    let mut reader = BufReader::new(keyfile);
    let mut ec_keys = rustls_pemfile::ec_private_keys(&mut reader)
        .filter_map(|res| res.ok());
    if let Some(key) = ec_keys.next() {
        info!("EC private key loaded from {path}");
        return Ok(PrivateKeyDer::Sec1(key));
    }
    error!("No private key found in {path} (neither PKCS#8, RSA, nor EC)");
    Err(IoError::new(std::io::ErrorKind::InvalidInput, "No private key found (neither PKCS#8, RSA, nor EC)"))
}

impl Webserver for Https1Server {
    fn set_listener(&self, listener: CallbackFn) {
        let web_listener = self.listener.clone();
        tokio::spawn(async move {
            let _ = web_listener.lock().await.insert(listener);
        });
    }
}

impl Service<Request<Incoming>> for Https1Server {
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

                let request = Https1Server::build_http_request(req).await;
                let response = listener(request).await.map_err(|err| {
                    ServerError::RequestProcessingError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("{:?}", err),
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

                let mut response_builder = Response::builder().status(status_code);
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

impl Https1Server {
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
        HttpRequest {
            request_method: method,
            path,
            host,
            headers,
            body: BASE64_STANDARD.encode(body_bytes),
        }
    }
}

#[tokio::test]
async fn test_https1server_with_self_signed_cert() {
    use rcgen::generate_simple_self_signed;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use hyper::{Request};
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;
    use hyper_rustls::{HttpsConnectorBuilder, ConfigBuilderExt};
    use http_body_util::Full;
    use bytes::Bytes;
    use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
    use rustls::client::danger::{ServerCertVerified, ServerCertVerifier, HandshakeSignatureValid};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use std::sync::Arc as StdArc;
    // use std::time::SystemTime;
    use crate::plugin_communication::models::{HttpRequest, HttpResponse};
    use crate::webserver::https_1_server::Https1Server;
    use crate::webserver::webserver::Webserver;
    use futures::FutureExt;
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use tokio::sync::Mutex;

    #[derive(Debug)]
    struct NoCertificateVerification;
    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer,
            _intermediates: &[CertificateDer],
            _server_name: &ServerName,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::RSA_PKCS1_SHA256,
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ECDSA_NISTP521_SHA512,
            ]
        }
    }

    // 1. Generate self-signed certificate
    let subject_alt_names = vec!["localhost".to_string()];
    let cert = generate_simple_self_signed(subject_alt_names).unwrap();
    let cert_pem = cert.serialize_pem().unwrap();
    let key_pem = cert.serialize_private_key_pem();

    // 2. Write to temporary files
    let mut cert_file = NamedTempFile::new().unwrap();
    let mut key_file = NamedTempFile::new().unwrap();
    cert_file.write_all(cert_pem.as_bytes()).unwrap();
    key_file.write_all(key_pem.as_bytes()).unwrap();

    // 3. Start server
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 44300);
    let server = Https1Server::start(
        addr,
        cert_file.path().to_str().unwrap(),
        key_file.path().to_str().unwrap(),
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // 4. Set listener
    let (sender, receiver) = tokio::sync::oneshot::channel::<HttpRequest>();
    let sender = Arc::new(Mutex::new(Some(sender)));
    server.set_listener(Box::new(move |request| {
        let sender = sender.clone();
        async move {
            sender.lock().await.take().unwrap().send(request).unwrap();
            Ok(HttpResponse {
                headers: vec![],
                status_code: 200,
                body: BASE64_STANDARD.encode("hello https"),
            })
        }
        .boxed()
    }));

    // 5. Build TLS client (accepts all certificates)
    let mut config = ClientConfig::builder()
        .with_native_roots().expect("native roots")
        .with_no_client_auth();
    config.dangerous().set_certificate_verifier(StdArc::new(NoCertificateVerification));
    let https = HttpsConnectorBuilder::new()
        .with_tls_config(config)
        .https_only()
        .enable_http1()
        .build();
    let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

    // 6. Send request
    let req = Request::builder()
        .uri("https://localhost:44300/test")
        .method("GET")
        .body(Full::new(Bytes::from("")))
        .unwrap();

    let response_result = tokio::time::timeout(std::time::Duration::from_secs(3), client.request(req)).await;
    match response_result {
        Ok(Ok(_response)) => { /* all ok */ }
        Ok(Err(e)) => {
            panic!("Client request error: {e:?}");
        },
        Err(_) => panic!("Timeout: Client request was not answered within 3 seconds!"),
    }
    let request = receiver.await.unwrap();
    assert_eq!(request.path, "/test");
}
