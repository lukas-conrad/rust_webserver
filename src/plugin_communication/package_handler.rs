use futures::future::BoxFuture;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

pub type ArrayListener = Box<dyn Fn(Vec<u8>) -> BoxFuture<'static, ()> + Send + Sync>;
pub struct PackageHandler {
    write: Mutex<Box<dyn AsyncWrite + Unpin + Send>>,
    listener: Mutex<ArrayListener>,
}

impl PackageHandler {
    pub fn new(
        read: Box<dyn AsyncRead + Unpin + Send>,
        write: Box<dyn AsyncWrite + Unpin + Send>,
        listener: ArrayListener,
    ) -> Arc<Self> {
        let package_handler = Arc::new(Self {
            write: Mutex::new(write),
            listener: Mutex::new(listener),
        });
        let package_handler_clone = package_handler.clone();
        tokio::spawn(async move {
            let read = Arc::new(Mutex::new(read));
            loop {
                match Self::read_package(read.clone()).await {
                    Ok(data) => {
                        let package_handler = &package_handler_clone.clone();
                        let listener = package_handler.listener.lock().await;
                        let _ = listener(data).await;
                    }
                    Err(err) => {
                        // TODO: Error handling
                        break;
                    }
                }
            }
        });

        package_handler
    }

    async fn read_package(
        read: Arc<Mutex<dyn AsyncRead + Unpin + Send>>,
    ) -> std::io::Result<Vec<u8>> {
        let mut size_buf = [0u8; 4];
        let mut read_guard = read.lock().await;
        read_guard.read_exact(&mut size_buf).await?;
        println!("Size Buffer: {:?}", size_buf);

        let len = u32::from_be_bytes(size_buf) as usize;
        let mut data_buf = Vec::with_capacity(len);
        data_buf.resize(len, 0);
        read_guard.read_exact(&mut data_buf).await?;

        Ok(data_buf)
    }

    pub async fn send_package(&self, package: &Vec<u8>) -> std::io::Result<()> {
        let len: u32 = package.len() as u32;
        let mut data = Vec::with_capacity(4 + len as usize);
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(package);

        let mut mutex_guard = self.write.lock().await;
        mutex_guard.write_all(&data).await?;
        mutex_guard.flush().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::plugin_communication::package_handler::PackageHandler;
    use futures::FutureExt;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::duplex;
    use tokio::sync::Mutex;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_send_receive() {
        let (client, server) = duplex(1024);
        let (read1, write1) = tokio::io::split(server);
        let (read2, write2) = tokio::io::split(client);

        let receiver: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));

        let receiver_clone = receiver.clone();

        let sender = PackageHandler::new(
            Box::new(read1),
            Box::new(write1),
            Box::new(|_| async {}.boxed()),
        );
        let _ = PackageHandler::new(
            Box::new(read2),
            Box::new(write2),
            Box::new(move |vec| {
                let receiver_clone = receiver_clone.clone();
                async move {
                    println!("received: {:?}", vec);
                    let _ = receiver_clone.lock().await.insert(vec);
                }
                .boxed()
            }),
        );
        // Sende Test-Daten
        sender.send_package(&vec![1, 2, 3]).await.unwrap();

        // Warte 100ms auf Empfang
        sleep(Duration::from_millis(100)).await;

        // Prüfe Ergebnis
        assert!(receiver.lock().await.is_some());
        assert_eq!(*receiver.lock().await, Some(vec![1, 2, 3]));

        sender.send_package(&vec![0u8; 100000]).await.unwrap();

        // Warte 100ms auf Empfang
        sleep(Duration::from_millis(100)).await;

        // Prüfe Ergebnis
        assert!(receiver.lock().await.is_some());
        assert_eq!(*receiver.lock().await, Some(vec![0u8; 100000]));
    }
}
