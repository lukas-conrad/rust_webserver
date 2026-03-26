use crate::config::DomainConfig;
use crate::webserver::cert_manager::CertificateManager;
use crate::webserver::webserver::{CallbackFn, ServerError, Webserver};
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use log::{debug, error, info, warn};
use std::convert::Infallible;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{RwLock};

use crate::webserver::utils::{build_http_request, build_http_response};
use hyper::server::conn::http1;
use tokio_rustls::rustls::{self};

/// HTTPS/1.1 Server implementation using Hyper and TLS
pub struct Https1Server {
    /// Callback function for handling incoming requests
    /// Uses RwLock for better concurrency - multiple readers, single writer
    listener: Arc<RwLock<Option<CallbackFn>>>,
}

impl Https1Server {
    /// Start HTTPS server with multi-domain SNI support
    pub async fn start(
        addr: SocketAddr,
        domains: Vec<DomainConfig>,
    ) -> Result<Arc<Self>, Box<dyn Error + Send + Sync>> {
        if domains.is_empty() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "At least one domain configuration is required",
            )));
        }

        let server = Arc::new(Self {
            listener: Arc::new(RwLock::new(None)),
        });

        // Build a single ServerConfig handling multiple domains via SNI
        let tls_acceptor = CertificateManager::create_updating_acceptor(&domains).await?;

        let listener = Arc::new(TcpListener::bind(addr).await?);

        spawn_cloned!(server; async move {
            loop {
                let server = server.clone();
                match listener.accept().await {
                    Ok((stream, _)) => {
                        spawn_cloned!(tls_acceptor, server; async move {
                            match tls_acceptor.read().await.accept(stream).await {
                                Ok(tls_stream) => {
                                    let io = TokioIo::new(tls_stream);
                                    if let Err(e) =
                                        http1::Builder::new().serve_connection(io, server).await
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

        info!(
            "Https1Server started on {} with {} domain(s)",
            addr,
            domains.len()
        );
        for domain in &domains {
            info!("  - {}", domain.domain);
        }

        Ok(server)
    }
}

impl Webserver for Https1Server {
    fn set_listener(&self, listener: CallbackFn) {
        let web_listener = self.listener.clone();
        tokio::spawn(async move {
            let _ = web_listener.write().await.insert(listener);
            info!("Request listener configured");
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
                // Use read lock instead of write - listener doesn't change after start
                let listener_guard = listener.read().await;
                let listener =
                    listener_guard
                        .as_ref()
                        .ok_or(ServerError::RequestProcessingError(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "No listener configured".to_string(),
                        ))?;

                let request = build_http_request(req).await;
                debug!(
                    "Processing request: {} {}",
                    request.request_method, request.path
                );

                let response = listener(request).await.map_err(|err| {
                    error!("Plugin error: {:?}", err);
                    ServerError::RequestProcessingError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Plugin error: {:?}", err),
                    )
                })?;

                let client_response = build_http_response(response)?;

                debug!("Response sent: status {}", client_response.status());
                Ok(client_response)
            }
            .await;

            match result {
                Ok(response) => Ok(response),
                Err(ServerError::RequestProcessingError(code, msg)) => {
                    warn!("Request error: {} - {}", code, msg);
                    Ok(Response::builder()
                        .status(code)
                        .body(Full::new(Bytes::from(msg)))
                        .unwrap_or_else(|_| {
                            Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .body(Full::new(Bytes::from("Internal Server Error")))
                                .unwrap()
                        }))
                }
            }
        })
    }
}

#[tokio::test]
async fn test_https1server_with_self_signed_cert() {
    use crate::plugin_communication::models::{HttpRequest, HttpResponse};
    use crate::webserver::https_1_server::Https1Server;
    use crate::webserver::webserver::Webserver;
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;
    use bytes::Bytes;
    use futures::FutureExt;
    use http_body_util::Full;
    use hyper::Request;
    use hyper_rustls::{ConfigBuilderExt, HttpsConnectorBuilder};
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;
    use rcgen::generate_simple_self_signed;
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
    use std::io::Write;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::Arc as StdArc;
    use tempfile::NamedTempFile;
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

    // 3. Start server on fixed test port
    let test_port = 44443;
    let addr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        test_port,
    );
    let domain_config = DomainConfig {
        domain: "localhost".to_string(),
        cert_path: cert_file.path().to_str().unwrap().to_string(),
        key_path: key_file.path().to_str().unwrap().to_string(),
    };
    let server = match Https1Server::start(addr, vec![domain_config]).await {
        Ok(s) => s,
        Err(e) => {
            // If port is in use, skip test
            eprintln!(
                "Could not bind to port {}: {}. Skipping test.",
                test_port, e
            );
            return;
        }
    };

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 4. Set listener
    let (sender, receiver) = tokio::sync::oneshot::channel::<HttpRequest>();
    let sender = Arc::new(Mutex::new(Some(sender)));
    server.set_listener(Box::new(move |request| {
        let sender = sender.clone();
        async move {
            if let Some(tx) = sender.lock().await.take() {
                let _ = tx.send(request);
            }
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
        .with_native_roots()
        .expect("native roots")
        .with_no_client_auth();
    config
        .dangerous()
        .set_certificate_verifier(StdArc::new(NoCertificateVerification));
    let https = HttpsConnectorBuilder::new()
        .with_tls_config(config)
        .https_only()
        .enable_http1()
        .build();
    let client: Client<_, Full<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

    // 6. Send HTTPS request to test server
    let req = Request::builder()
        .uri(format!("https://localhost:{}/test", test_port))
        .method("GET")
        .body(Full::new(Bytes::from("")))
        .unwrap();

    let response_result =
        tokio::time::timeout(std::time::Duration::from_secs(3), client.request(req)).await;
    match response_result {
        Ok(Ok(_response)) => { /* all ok */ }
        Ok(Err(e)) => {
            panic!("Client request error: {e:?}");
        }
        Err(_) => panic!("Timeout: Client request was not answered within 3 seconds!"),
    }
    let request = receiver.await.unwrap();
    assert_eq!(request.path, "/test");
}
