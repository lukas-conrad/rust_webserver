use crate::plugin_communication::package_handler::PackageHandler;
use crate::plugin_communication::plugin_communicator::CommunicationError::SendingFailed;
use crate::plugin_old::models::Package;
use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::{AsyncRead, FutureExt};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

pub type Listener = Box<dyn Fn(Package) -> BoxFuture<'static, ()> + Send + Sync>;
pub type Filter = Box<dyn Fn(&Package) -> bool + Send + Sync>;

pub enum CommunicationError {
    TimeoutError(String),
    SendingFailed(String),
}

#[async_trait]
pub trait PluginCommunicator: Send + Sync {
    async fn set_listener(&mut self, listener: Listener);

    async fn send_package(
        &self,
        package: Package,
        filter: Option<Filter>,
    ) -> Result<Option<Package>, CommunicationError>;
}

struct DefaultCommunicator {
    package_handler: Arc<PackageHandler>,
    package_listener: Arc<Mutex<Option<Listener>>>,
    response_listener: Arc<Mutex<Vec<(Filter, tokio::sync::oneshot::Sender<Package>)>>>,
}

impl DefaultCommunicator {
    fn new(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
    ) -> Self {
        let package_listener = Arc::new(Mutex::new(None::<Listener>));
        let package_listener_clone = package_listener.clone();
        let response_listener = Arc::new(Mutex::new(Vec::<(
            Filter,
            tokio::sync::oneshot::Sender<Package>,
        )>::new()));
        let response_listener_clone = response_listener.clone();
        Self {
            package_handler: PackageHandler::new(
                read,
                write,
                Box::new(move |data: Vec<u8>| {
                    let package_listener_clone = package_listener_clone.clone();
                    let response_listener_clone = response_listener_clone.clone();
                    Self::process_package(data, package_listener_clone, response_listener_clone)
                        .boxed()
                }),
            ),
            package_listener,
            response_listener,
        }
    }

    async fn process_package(
        data: Vec<u8>,
        package_listener_clone: Arc<Mutex<Option<Listener>>>,
        response_listener_clone: Arc<Mutex<Vec<(Filter, tokio::sync::oneshot::Sender<Package>)>>>,
    ) {
        match serde_json::from_slice::<Package>(data.as_slice()) {
            Ok(package) => {
                let mut vec_guard = response_listener_clone.lock().await;
                let vec = vec_guard.deref_mut();
                let pos = vec.iter().position(|(filter, _)| filter(&package));

                // check response listener
                if let Some(pos) = pos {
                    let (_, sender) = vec.remove(pos);
                    match sender.send(package) {
                        Err(err) => {
                            // TODO: error handling, sending package back failed
                        }
                        Ok(_) => {}
                    }
                }
                // otherwise, use normal listener
                else if let Some(listener) = package_listener_clone.lock().await.deref() {
                    let _ = listener(package).await;
                }
            }
            Err(err) => {
                // TODO: Error handling (package deserialisation)
            }
        }
    }
}

#[async_trait]
impl PluginCommunicator for DefaultCommunicator {
    async fn set_listener(&mut self, listener: Listener) {
        let _ = self.package_listener.lock().await.insert(listener);
    }

    async fn send_package(
        &self,
        package: Package,
        filter: Option<Filter>,
    ) -> Result<Option<Package>, CommunicationError> {
        if let Some(filter) = filter {
            let (sender, receiver) = tokio::sync::oneshot::channel::<Package>();
            self.response_listener.lock().await.push((filter, sender));
            return receiver
                .await
                .map(|package| Some(package))
                .map_err(|e| SendingFailed(e.to_string()));
        }
        self.package_handler
            .send_package(&serde_json::to_vec(&package).unwrap())
            .await
            .map_err(|e| SendingFailed(e.to_string()))?;
        Ok(None)
    }
}
