use crate::plugin::interfaces::{PackageHandlerError, PluginCommunicator};
use crate::plugin::models::{
    Package, PackageGen, PackageHandshakeRequest, PackageHandshakeResponse, PackageNormalRequest, PackageNormalResponse,
    PackageType, PluginConfig,
};
use crate::plugin::PackageHandler;
use log::{error, info};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;

pub type CallbackFn = Box<dyn Fn(Package) + Send + Sync + 'static>;

pub struct AsyncPluginCommunicator {
    pub package_handler: Box<dyn PackageHandler>,
    waiting_handles: Arc<Mutex<HashMap<i64, oneshot::Sender<PackageNormalResponse>>>>,
    handshake_request: Arc<Mutex<Option<oneshot::Sender<PackageHandshakeResponse>>>>,
    pub plugin_config: Arc<PluginConfig>,
}

impl AsyncPluginCommunicator {
    pub async fn new(
        plugin_config: Arc<PluginConfig>,
        handler: Box<dyn PackageHandler>,
        package_callback: CallbackFn,
    ) -> Self {
        info!(
            "Creating new AsyncPluginCommunicator for plugin: {}",
            plugin_config.plugin_name
        );
        let waiting_handles = Arc::new(Mutex::new(HashMap::new()));
        let waiting_handles_clone = waiting_handles.clone();
        let package_callback_arc = Arc::new(Mutex::new(package_callback));
        let handshake_request = Arc::new(Mutex::new(None));

        let mut result = Self {
            package_handler: handler,
            waiting_handles,
            handshake_request: handshake_request.clone(),
            plugin_config,
        };

        // Use a reference to the struct in the callback
        result
            .package_handler
            .set_callback_function(Box::new(move |bytes| {
                let handles_clone = waiting_handles_clone.clone();
                let handshake_request_clone = handshake_request.clone();
                // info!("Received package: {}", String::from_utf8_lossy(bytes));
                let res = serde_json::from_slice::<Package>(bytes);
                match res {
                    Ok(package) => {
                        if matches!(package, Package::NormalResponse(_)) {
                            if let Package::NormalResponse(content) = package {
                                let package_id = content.package_id;

                                tokio::spawn(async move {
                                    let mut map = handles_clone.lock().await;
                                    if let Some(sender) = map.remove(&package_id) {
                                        let response = PackageNormalResponse::new(content);
                                        let _ = sender.send(response);
                                    }
                                });
                            }
                        } else if matches!(package, Package::HandshakeResponse(_))
                            || matches!(package, Package::HandshakeRequest(_))
                        {
                            let package_content = package.clone();
                            if let Package::HandshakeResponse(content) = package_content {
                                tokio::spawn(async move {
                                    if let Some(sender) =
                                        handshake_request_clone.lock().await.take()
                                    {
                                        let _ = sender.send(PackageHandshakeResponse::new(content));
                                    }
                                });
                            }
                        } else {
                            let callback_clone = package_callback_arc.clone();
                            tokio::spawn(async move {
                                let callback = callback_clone.lock().await;
                                callback(package);
                            });
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse package: {:?}", e);
                    }
                }
            }))
            .await;

        info!(
            "Reader loop for plugin {} started",
            result.plugin_config.plugin_name
        );
        result.package_handler.start_reader_loop();
        result
    }
}

#[async_trait]
impl PluginCommunicator for AsyncPluginCommunicator {
    async fn send_request(
        &self,
        package: PackageNormalRequest,
    ) -> Result<PackageNormalResponse, PackageHandlerError> {
        let (sender, receiver) = oneshot::channel::<PackageNormalResponse>();

        let package_id = package.content.package_id;
        // info!("Sending request package with ID {}", package_id);
        self.waiting_handles.lock().await.insert(package_id, sender);

        self.send_package(package.to_package())?;

        match timeout(
            Duration::from_millis(self.plugin_config.max_request_timeout),
            receiver,
        )
        .await
        {
            Ok(result) => match result {
                Ok(response) => {
                    // info!("Response for package {} received", package_id);
                    Ok(response)
                }
                Err(_) => {
                    info!(
                        "Communication channel for package {} unexpectedly closed",
                        package_id
                    );
                    Err(PackageHandlerError::SendingFailed(
                        "The communication channel was unexpectedly closed".to_string(),
                    ))
                }
            },
            Err(_) => {
                info!(
                    "Timeout for package {} after {}ms",
                    package_id, self.plugin_config.max_request_timeout
                );
                self.waiting_handles.lock().await.remove(&package_id);

                Err(PackageHandlerError::SendingFailed(format!(
                    "Timeout after {}ms waiting for response for package {}",
                    self.plugin_config.max_request_timeout, package_id
                )))
            }
        }
    }

    fn send_package(&self, package: Package) -> Result<(), PackageHandlerError> {
        let json_string = serde_json::to_string(&package)
            .map_err(|e| PackageHandlerError::SerializationError(e.to_string()))?;

        let json_bytes = json_string.into_bytes();
        self.package_handler.send_package(json_bytes)
    }

    async fn send_handshake(
        &self,
        package: PackageHandshakeRequest,
    ) -> Result<PackageHandshakeResponse, PackageHandlerError> {
        info!(
            "Sending handshake request to plugin {}",
            self.plugin_config.plugin_name
        );
        let (sender, receiver) = oneshot::channel::<PackageHandshakeResponse>();

        let _ = self.handshake_request.lock().await.insert(sender);

        self.send_package(package.to_package())?;

        let timeout_duration = self.plugin_config.max_startup_time;
        match timeout(Duration::from_millis(timeout_duration), receiver).await {
            Ok(result) => {
                match result {
                    Ok(response) => {
                        info!(
                            "Handshake response successfully received from plugin {}",
                            self.plugin_config.plugin_name
                        );
                        Ok(response)
                    }
                    Err(_) => {
                        info!("Communication channel for handshake with plugin {} unexpectedly closed", self.plugin_config.plugin_name);
                        Err(PackageHandlerError::SendingFailed(
                            "The communication channel was unexpectedly closed".to_string(),
                        ))
                    }
                }
            }
            Err(_) => {
                info!(
                    "Timeout while handshaking with plugin {} after {}ms",
                    self.plugin_config.plugin_name, timeout_duration
                );
                let _ = self.handshake_request.lock().await.take();

                Err(PackageHandlerError::SendingFailed(format!(
                    "Timeout after {}ms waiting for response for package",
                    timeout_duration,
                )))
            }
        }
    }
}
