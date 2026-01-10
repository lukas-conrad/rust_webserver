use crate::plugin_communication::package_handler::PackageHandler;
use crate::plugin_communication::plugin_communicator::CommunicationError::SendingFailed;
use crate::plugin_old::models::Package;
use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;

pub type Listener = Box<dyn Fn(Package) -> BoxFuture<'static, ()> + Send + Sync>;
pub type Filter = Box<dyn Fn(&Package) -> bool + Send + Sync>;

#[derive(Debug)]
pub enum CommunicationError {
    TimeoutError(String),
    SendingFailed(String),
}

#[async_trait]
pub trait PluginCommunicator: Send + Sync {
    async fn set_listener(&mut self, listener: Listener);

    async fn send_package(
        &self,
        package: &Package,
        filter: Option<Filter>,
    ) -> Result<Option<Package>, CommunicationError>;
}

pub struct JsonCommunicator {
    package_handler: Arc<PackageHandler>,
    package_listener: Arc<Mutex<Option<Listener>>>,
    response_listener: Arc<Mutex<Vec<(Filter, tokio::sync::oneshot::Sender<Package>)>>>,
}

impl JsonCommunicator {
    pub fn new(
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
        let package_handler = PackageHandler::new(
            read,
            write,
            Box::new(move |data: Vec<u8>| {
                Self::process_package(
                    data,
                    package_listener_clone.clone(),
                    response_listener_clone.clone(),
                )
                .boxed()
            }),
        );
        Self {
            package_handler,
            package_listener,
            response_listener,
        }
    }

    async fn process_package(
        data: Vec<u8>,
        package_listener: Arc<Mutex<Option<Listener>>>,
        response_listener: Arc<Mutex<Vec<(Filter, tokio::sync::oneshot::Sender<Package>)>>>,
    ) {
        match serde_json::from_slice::<Package>(data.as_slice()) {
            Ok(package) => {
                let mut vec_guard = response_listener.lock().await;
                let vec = vec_guard.deref_mut();
                let pos = vec.iter().position(|(filter, _)| filter(&package));

                // check response listener
                if let Some(pos) = pos {
                    let (_, sender) = vec.remove(pos);
                    let _ = sender.send(package);
                }
                // otherwise, use normal listener
                else if let Some(listener) = package_listener.lock().await.deref() {
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
impl PluginCommunicator for JsonCommunicator {
    async fn set_listener(&mut self, listener: Listener) {
        let _ = self.package_listener.lock().await.insert(listener);
    }

    async fn send_package(
        &self,
        package: &Package,
        filter: Option<Filter>,
    ) -> Result<Option<Package>, CommunicationError> {
        if let Some(filter) = filter {
            let (sender, receiver) = tokio::sync::oneshot::channel::<Package>();
            self.response_listener.lock().await.push((filter, sender));
            self.package_handler
                .send_package(&serde_json::to_vec(&package).unwrap())
                .await
                .map_err(|e| SendingFailed(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_old::models::{LogContent, Package};
    use tokio::io::duplex;
    use tokio::time::{sleep, timeout, Duration};

    fn create_test_package(id: i64, message: &str) -> Package {
        Package::Log(LogContent {
            level: id.to_string(),
            message: message.to_string(),
        })
    }

    fn get_log_level(package: &Package) -> i64 {
        match package {
            Package::Log(content) => content.level.parse().unwrap_or(0),
            _ => 0,
        }
    }

    fn get_package_message(package: &Package) -> String {
        match package {
            Package::Log(content) => content.message.clone(),
            _ => String::new(),
        }
    }

    #[tokio::test]
    async fn test_send_and_receive_package() {
        // Setup duplex stream
        let (client, server) = duplex(1024);
        let (read1, write1) = tokio::io::split(client);
        let (read2, write2) = tokio::io::split(server);

        // Create two communicators
        let comm1 = JsonCommunicator::new(Box::new(read1), Box::new(write1));
        let mut comm2 = JsonCommunicator::new(Box::new(read2), Box::new(write2));

        // Setup receiver for comm2
        let received: Arc<Mutex<Option<Package>>> = Arc::new(Mutex::new(None));
        let received_clone = received.clone();

        comm2
            .set_listener(Box::new(move |package| {
                let received_clone = received_clone.clone();
                async move {
                    println!("Received package: {:?}", package);
                    let _ = received_clone.lock().await.insert(package);
                }
                .boxed()
            }))
            .await;

        // Send package from comm1
        let test_package = create_test_package(1, "Hello World");
        comm1
            .send_package(&test_package, None)
            .await
            .expect("Failed to send package");

        // Wait for package to be received
        sleep(Duration::from_millis(100)).await;

        // Verify package was received
        let received_package = received.lock().await;
        assert!(received_package.is_some(), "No package received");
        assert_eq!(
            get_log_level(received_package.as_ref().unwrap()),
            1,
            "Package log level mismatch"
        );
        assert_eq!(
            get_package_message(received_package.as_ref().unwrap()),
            "Hello World",
            "Package message mismatch"
        );
    }

    #[tokio::test]
    async fn test_send_package_with_filter_response() {
        // Setup duplex stream
        let (client, server) = duplex(1024);
        let (read1, write1) = tokio::io::split(client);
        let (read2, write2) = tokio::io::split(server);

        // Create two communicators
        let comm1 = JsonCommunicator::new(Box::new(read1), Box::new(write1));
        let comm2 = JsonCommunicator::new(Box::new(read2), Box::new(write2));

        // Setup comm2 to automatically respond to incoming packages
        let comm2_arc = Arc::new(comm2);
        let comm2_clone = comm2_arc.clone();

        // We need to set listener before wrapping in Arc, so we need a different approach
        let (client2, server2) = duplex(1024);
        let (read1, write1) = tokio::io::split(client2);
        let (read2, write2) = tokio::io::split(server2);

        let comm1 = JsonCommunicator::new(Box::new(read1), Box::new(write1));
        let comm2 = Arc::new(Mutex::new(JsonCommunicator::new(
            Box::new(read2),
            Box::new(write2),
        )));

        let comm1_arc = Arc::new(comm1);

        comm2
            .clone()
            .lock()
            .await
            .set_listener(Box::new(move |package| {
                let comm2 = comm2.clone();
                async move {
                    println!("Comm2 received request package: {:?}", package);

                    let request_level = get_log_level(&package);
                    let request_message = get_package_message(&package);

                    // Send response package
                    let response = create_test_package(
                        request_level + 100,
                        &format!("Response to: {}", request_message),
                    );

                    tokio::spawn(async move {
                        comm2
                            .lock()
                            .await
                            .send_package(&response, None)
                            .await
                            .expect("Failed to send response");
                    });
                }
                .boxed()
            }))
            .await;

        // Send package from comm1 with filter waiting for response
        let request_package = create_test_package(42, "Request");

        let filter: Filter = Box::new(|pkg| get_log_level(pkg) == 142); // Expect response with level 142 (42 + 100)

        // Send with timeout
        let result = tokio::select! {
            result = comm1_arc.send_package(&request_package, Some(filter)) => result,
            _ = sleep(Duration::from_millis(100)) => Err(CommunicationError::TimeoutError("Timeout".to_string()))
        };

        assert!(result.is_ok(), "Request timed out");
        let response = result.unwrap();

        assert!(response.is_some(), "No response received");
        let response_package = response.unwrap();
        assert_eq!(
            get_log_level(&response_package),
            142,
            "Response log level should be 142 (request level 42 + 100)"
        );
        assert_eq!(
            get_package_message(&response_package),
            "Response to: Request",
            "Response message mismatch"
        );
    }

    #[tokio::test]
    async fn test_multiple_packages_send_receive() {
        // Setup duplex stream
        let (client, server) = duplex(4096);
        let (read1, write1) = tokio::io::split(client);
        let (read2, write2) = tokio::io::split(server);

        // Create two communicators
        let mut comm1 = JsonCommunicator::new(Box::new(read1), Box::new(write1));
        let mut comm2 = JsonCommunicator::new(Box::new(read2), Box::new(write2));

        // Setup receiver for comm2
        let received: Arc<Mutex<Vec<Package>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        comm2
            .set_listener(Box::new(move |package| {
                let received_clone = received_clone.clone();
                async move {
                    received_clone.lock().await.push(package);
                }
                .boxed()
            }))
            .await;

        // Send multiple packages
        for i in 0..5 {
            let package = create_test_package(i, &format!("Message {}", i));
            comm1
                .send_package(&package, None)
                .await
                .expect("Failed to send package");
        }

        // Wait for all packages to be received
        sleep(Duration::from_millis(200)).await;

        // Verify all packages were received
        let received_packages = received.lock().await;
        assert_eq!(received_packages.len(), 5, "Should receive 5 packages");

        for i in 0..5 {
            assert!(
                received_packages.iter().any(|p| get_log_level(p) == i),
                "Package with log level {} not found",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_filter_timeout_when_no_response() {
        // Setup duplex stream
        let (client, server) = duplex(1024);
        let (read1, write1) = tokio::io::split(client);
        let (read2, write2) = tokio::io::split(server);

        // Create two communicators
        let comm1 = JsonCommunicator::new(Box::new(read1), Box::new(write1));
        let comm1_arc = Arc::new(comm1);
        let mut comm2 = JsonCommunicator::new(Box::new(read2), Box::new(write2));

        // Setup comm2 to NOT respond (just receive)
        comm2
            .set_listener(Box::new(move |package| {
                async move {
                    println!("Comm2 received but will not respond: {:?}", package);
                }
                .boxed()
            }))
            .await;

        // Send package with filter expecting response that will never come
        let request_package = create_test_package(99, "Request without response");

        let filter: Filter = Box::new(|pkg| get_log_level(pkg) == 999);

        // Send with short timeout - should timeout
        let result = timeout(
            Duration::from_millis(100),
            comm1_arc.send_package(&request_package, Some(filter)),
        )
        .await;

        assert!(
            result.is_err(),
            "Should have timed out waiting for response"
        );
    }
}
