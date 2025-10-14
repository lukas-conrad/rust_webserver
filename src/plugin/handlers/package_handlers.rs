use crate::plugin::interfaces::CallbackFn;
use crate::plugin::interfaces::PackageHandlerError;
use crate::plugin::PackageHandler;
use log::{error, info};
use std::future::Future;
use std::pin::Pin;

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub struct AsyncPackageHandler<W, R>
where
    W: AsyncWriteExt + Unpin + Send + Sync + 'static,
    R: AsyncReadExt + Unpin + Send + Sync + 'static,
{
    writer: Arc<Mutex<W>>,
    reader: Arc<Mutex<R>>,
    callback: Arc<Mutex<Option<CallbackFn>>>,
}

impl<W, R> AsyncPackageHandler<W, R>
where
    W: AsyncWriteExt + Unpin + Send + Sync + 'static,
    R: AsyncReadExt + Unpin + Send + Sync + 'static,
{
    pub fn new(writer: W, reader: R) -> Self {
        Self {
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            callback: Arc::new(Mutex::new(None)),
        }
    }
}

impl<W, R> PackageHandler for AsyncPackageHandler<W, R>
where
    W: AsyncWriteExt + Unpin + Send + Sync + 'static,
    R: AsyncReadExt + Unpin + Send + Sync + 'static,
{
    fn send_package(&self, data: Vec<u8>) -> Result<(), PackageHandlerError> {
        // info!("Sende Paket mit {} Bytes", data.len());
        let writer = self.writer.clone();
        let data_len = data.len() as u32;

        let mut package_with_header = Vec::with_capacity(4 + data.len());

        package_with_header.extend_from_slice(&data_len.to_be_bytes());
        package_with_header.extend_from_slice(&data);

        tokio::spawn(async move {
            let mut writer_guard = writer.lock().await;
            if let Err(e) = writer_guard.write_all(&package_with_header).await {
                eprintln!("Fehler beim Senden: {}", e);
            }
            if let Err(e) = writer_guard.flush().await {
                eprintln!("Fehler beim Senden {}", e)
            }
        });

        Ok(())
    }

    fn set_callback_function(&mut self, callback: CallbackFn) -> Pin<Box<dyn Future<Output = ()>>> {
        let arc = self.callback.clone();
        Box::pin(async move {
            let _ = arc.lock().await.insert(callback);
        })
    }

    fn start_reader_loop(&self) {
        let reader = self.reader.clone();
        let callback = self.callback.clone();
        tokio::spawn(async move {
            let mut length_buffer = [0u8; 4];

            loop {
                let length_result = async {
                    let mut reader_guard = reader.lock().await;
                    reader_guard.read_exact(&mut length_buffer).await
                }
                .await;

                match length_result {
                    Ok(_) => {
                        let packet_length = u32::from_be_bytes(length_buffer);
                        // info!("Paketlänge erkannt: {} Bytes", packet_length);

                        let mut data_buffer = vec![0u8; packet_length as usize];

                        let read_result = async {
                            let mut reader_guard = reader.lock().await;
                            reader_guard.read_exact(&mut data_buffer).await
                        }
                        .await;

                        match read_result {
                            Ok(_) => {
                                let callback_guard = callback.lock().await;
                                if let Some(callback) = &*callback_guard {
                                    callback(&data_buffer);
                                } else {
                                    error!("Cannot read package: No callback set!")
                                }
                            }
                            Err(e) => {
                                info!("Fehler beim Lesen der Paketdaten: {}", e);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            info!("Reader-Loop beendet: EOF erreicht");
                            break;
                        }

                        info!("Fehler beim Lesen des Längenheaders: {}", e);
                        continue;
                    }
                }
            }
        });
    }
}
