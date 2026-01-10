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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
        sender.send_package(&vec![1, 2, 3]).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        assert!(receiver.lock().await.is_some());
        assert_eq!(*receiver.lock().await, Some(vec![1, 2, 3]));

        sender.send_package(&vec![0u8; 100000]).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        assert!(receiver.lock().await.is_some());
        assert_eq!(*receiver.lock().await, Some(vec![0u8; 100000]));
    }

    #[tokio::test]
    async fn test_send_receive_with_oneshot() {
        let (client, server) = duplex(1024);
        let (read1, write1) = tokio::io::split(server);
        let (read2, write2) = tokio::io::split(client);

        let (tx, rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let sender = PackageHandler::new(
            Box::new(read1),
            Box::new(write1),
            Box::new(|_| async {}.boxed()),
        );
        let _ = PackageHandler::new(
            Box::new(read2),
            Box::new(write2),
            Box::new(move |vec| {
                let tx_clone = tx.clone();
                async move {
                    println!("received via oneshot: {:?}", vec);
                    if let Some(sender) = tx_clone.lock().await.take() {
                        let _ = sender.send(vec);
                    }
                }
                .boxed()
            }),
        );

        let test_data = vec![5, 10, 15, 20];
        sender.send_package(&test_data).await.unwrap();

        let received = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Timeout: Did not received any data")
            .expect("Channel closed without data");

        assert_eq!(received, test_data);
    }

    #[tokio::test]
    async fn test_decoding_small_data() {
        let (mut raw_stream, handler_stream) = duplex(1024);
        let (read, write) = tokio::io::split(handler_stream);

        let received: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let received_clone = received.clone();

        let _handler = PackageHandler::new(
            Box::new(read),
            Box::new(write),
            Box::new(move |data| {
                let received_clone = received_clone.clone();
                async move {
                    let _ = received_clone.lock().await.insert(data);
                }
                .boxed()
            }),
        );

        // Manually encode and write to raw stream
        let test_data = vec![1, 2, 3, 4, 5];
        let len = test_data.len() as u32;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&len.to_be_bytes());
        encoded.extend_from_slice(&test_data);

        raw_stream.write_all(&encoded).await.unwrap();
        raw_stream.flush().await.unwrap();

        // Wait for PackageHandler to receive and decode
        sleep(Duration::from_millis(100)).await;

        // Verify decoded data
        let received_data = received.lock().await;
        assert!(received_data.is_some(), "No data received");
        assert_eq!(*received_data, Some(test_data));
    }

    #[tokio::test]
    async fn test_encoding_small_data() {
        let (mut raw_stream, handler_stream) = duplex(1024);
        let (read, write) = tokio::io::split(handler_stream);

        let handler = PackageHandler::new(
            Box::new(read),
            Box::new(write),
            Box::new(|_| async {}.boxed()),
        );

        // Send data via PackageHandler
        let test_data = vec![10, 20, 30, 40, 50];
        handler.send_package(&test_data).await.unwrap();

        // Manually read and decode from raw stream
        let mut len_buf = [0u8; 4];
        raw_stream.read_exact(&mut len_buf).await.unwrap();
        let decoded_len = u32::from_be_bytes(len_buf) as usize;

        assert_eq!(decoded_len, test_data.len(), "Length mismatch");

        let mut data_buf = vec![0u8; decoded_len];
        raw_stream.read_exact(&mut data_buf).await.unwrap();

        assert_eq!(data_buf, test_data, "Data mismatch");
    }

    #[tokio::test]
    async fn test_decoding_large_data() {
        let (mut raw_stream, handler_stream) = duplex(200_000);
        let (read, write) = tokio::io::split(handler_stream);

        let received: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let received_clone = received.clone();

        let _handler = PackageHandler::new(
            Box::new(read),
            Box::new(write),
            Box::new(move |data| {
                let received_clone = received_clone.clone();
                async move {
                    let _ = received_clone.lock().await.insert(data);
                }
                .boxed()
            }),
        );

        // Manually encode large data
        let test_data = vec![42u8; 100_000];
        let len = test_data.len() as u32;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&len.to_be_bytes());
        encoded.extend_from_slice(&test_data);

        raw_stream.write_all(&encoded).await.unwrap();
        raw_stream.flush().await.unwrap();

        // Wait for decoding
        sleep(Duration::from_millis(200)).await;

        // Verify
        let received_data = received.lock().await;
        assert!(received_data.is_some(), "No large data received");
        assert_eq!(received_data.as_ref().unwrap().len(), 100_000);
        assert_eq!(*received_data, Some(test_data));
    }

    #[tokio::test]
    async fn test_decoding_empty_data() {
        let (mut raw_stream, handler_stream) = duplex(1024);
        let (read, write) = tokio::io::split(handler_stream);

        let received: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let received_clone = received.clone();

        let _handler = PackageHandler::new(
            Box::new(read),
            Box::new(write),
            Box::new(move |data| {
                let received_clone = received_clone.clone();
                async move {
                    let _ = received_clone.lock().await.insert(data);
                }
                .boxed()
            }),
        );

        // Manually encode empty data
        let test_data: Vec<u8> = vec![];
        let len = test_data.len() as u32;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&len.to_be_bytes());
        encoded.extend_from_slice(&test_data);

        raw_stream.write_all(&encoded).await.unwrap();
        raw_stream.flush().await.unwrap();

        // Wait for decoding
        sleep(Duration::from_millis(100)).await;

        // Verify
        let received_data = received.lock().await;
        assert!(received_data.is_some(), "No empty data received");
        assert_eq!(*received_data, Some(vec![]));
    }
}
