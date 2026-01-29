use crate::plugin_communication::models::Package::{HandshakeResponse, NormalResponse};
use crate::plugin_communication::models::{
    HandshakeResponseContent, HttpResponse, NormalResponseContent, Package,
};
use crate::plugin_communication::plugin_communicator::{
    CommunicationError, Filter, JsonCommunicator, PluginCommunicator,
};
use futures::future::BoxFuture;
use futures::FutureExt;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, RwLock};

/// Configuration for handshake response
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    pub response_code: u32,
    pub response_code_text: String,
}

impl HandshakeConfig {
    pub fn success() -> Self {
        Self {
            response_code: 0,
            response_code_text: "all good".to_string(),
        }
    }

    pub fn failure(code: u32, message: String) -> Self {
        Self {
            response_code: code,
            response_code_text: message,
        }
    }
}

pub type PackageListener =
    Box<dyn Fn(&Package) -> BoxFuture<Option<Package>> + Send + Sync + 'static>;

pub struct TestPlugin {
    communicator: Arc<Mutex<Box<dyn PluginCommunicator>>>,
    listener: Arc<RwLock<Option<PackageListener>>>,
}

impl TestPlugin {
    pub async fn new(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
        listener: Option<PackageListener>,
    ) -> Self {
        Self::new_with_config(read, write, HandshakeConfig::success(), listener).await
    }

    pub async fn new_with_config(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
        handshake_config: HandshakeConfig,
        listener: Option<PackageListener>,
    ) -> Self {
        let communicator: Arc<Mutex<Box<dyn PluginCommunicator>>> =
            Arc::new(Mutex::new(Box::new(JsonCommunicator::new(read, write))));

        let listener = Arc::new(RwLock::new(listener));

        let communicator_clone = communicator.clone();
        let listener_clone = listener.clone();
        communicator
            .lock()
            .await
            .set_listener(Box::new(move |package| {
                let communicator_clone = communicator_clone.clone();
                let listener_clone = listener_clone.clone();
                let handshake_config = handshake_config.clone();
                tokio::spawn(async move {
                    let guard = listener_clone.read().await;
                    let listener_option = guard.as_ref();
                    let response = match listener_option {
                        Some(listener) => listener(&package).await,
                        None => None,
                    };

                    if let Some(package) = response {
                        communicator_clone
                            .lock()
                            .await
                            .send_package(&package, None)
                            .await
                            .unwrap();
                        return;
                    }

                    Self::default_response(package, communicator_clone, handshake_config).await;

                });
                async {}.boxed()
            }))
            .await;

        Self {
            communicator,
            listener,
        }
    }

    async fn default_response(
        package: Package,
        communicator_clone: Arc<Mutex<Box<dyn PluginCommunicator>>>,
        handshake_config: HandshakeConfig,
    ) {
        match package {
            Package::HandshakeRequest(_) => {
                communicator_clone
                    .lock()
                    .await
                    .send_package(
                        &HandshakeResponse(HandshakeResponseContent {
                            response_code: handshake_config.response_code,
                            response_code_text: handshake_config.response_code_text,
                        }),
                        None,
                    )
                    .await
                    .unwrap();
            }
            Package::NormalRequest(content) => {
                communicator_clone
                    .lock()
                    .await
                    .send_package(
                        &NormalResponse(NormalResponseContent {
                            package_id: content.package_id,
                            http_response: HttpResponse {
                                headers: vec![],
                                status_code: 200,
                                body: content.http_request.body,
                            },
                        }),
                        None,
                    )
                    .await
                    .unwrap();
            }
            _ => {}
        }
    }

    pub async fn set_listener(&self, listener: PackageListener) {
        let _ = self.listener.write().await.insert(listener);
    }

    pub async fn send_package(
        &self,
        package: &Package,
        filter: Option<Filter>,
    ) -> Result<Option<Package>, CommunicationError> {
        self.communicator
            .lock()
            .await
            .send_package(package, filter)
            .await
    }
}
