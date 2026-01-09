use futures::{AsyncRead, AsyncReadExt};
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;

pub type ArrayListener = Box<dyn Fn(Vec<u8>) + Send>;
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
                        listener(data);
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

        let len = u32::from_be_bytes(size_buf);
        let mut data_buf: Vec<u8> = Vec::with_capacity(len as usize);
        read_guard.read_exact(&mut data_buf).await?;

        Ok(data_buf)
    }

    pub async fn send_package(&self, package: &Vec<u8>) -> std::io::Result<()> {
        let len = package.len();
        let mut data = Vec::with_capacity(4 + len);
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(package);

        let mut mutex_guard = self.write.lock().await;
        mutex_guard.write_all(&data).await?;
        mutex_guard.flush().await?;

        Ok(())
    }
}
