use crate::plugin_communication::plugin_communicator::{
    CommunicationError, Filter, JsonCommunicator, PluginCommunicator,
};
use crate::plugin_old::models::Package::{HandshakeResponse, NormalResponse};
use crate::plugin_old::models::{
    HandshakeResponseContent, HttpResponse, NormalResponseContent, Package,
};
use futures::FutureExt;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;

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

pub struct TestPlugin {
    communicator: Arc<Mutex<Box<dyn PluginCommunicator>>>,
}

impl TestPlugin {
    pub async fn new(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
    ) -> Self {
        Self::new_with_config(read, write, HandshakeConfig::success()).await
    }

    pub async fn new_with_config(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
        handshake_config: HandshakeConfig,
    ) -> Self {
        let communicator: Arc<Mutex<Box<dyn PluginCommunicator>>> =
            Arc::new(Mutex::new(Box::new(JsonCommunicator::new(read, write))));

        let communicator_clone = communicator.clone();
        communicator
            .lock()
            .await
            .set_listener(Box::new(move |package| {
                let communicator_clone = communicator_clone.clone();
                let handshake_config = handshake_config.clone();
                async move {
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
                .boxed()
            }))
            .await;

        Self { communicator }
    }

    async fn send_package(
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
