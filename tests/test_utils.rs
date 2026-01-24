use std::io::{BufRead, BufReader};
use std::thread::JoinHandle;
use hyper::body::Incoming;
use hyper::client::conn::http1::SendRequest;
use hyper::Response;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead};
use tokio::net::TcpStream;
use std::process::Child;
use http_body_util::BodyExt;

/// Print stdout/stderr in real-time with a prefix
/// Returns a JoinHandle that can be used to abort the task
pub fn print_stdio<R: Unpin + Send + 'static + std::io::Read>(
    stream: R,
    prefix: String,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            match line {
                Ok(line) => println!("{} {}", prefix, line),
                Err(e) => {
                    eprintln!("{} Read error: {}", prefix, e);
                    break;
                }
            }
        }
    })
}

/// Check if a process is still running
/// Panics if the process has already terminated
pub fn check_server_running(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(status)) => {
            panic!("Server was immediately terminated with status: {}", status);
        }
        Ok(None) => println!("Server is running..."),
        Err(e) => panic!("Error checking server status: {}", e),
    }
}

/// Setup HTTP/1 handshake and return the sender
/// Spawns the connection task in the background
/// Generic over body type B
pub async fn setup_sender<B>(port: u16) -> SendRequest<B> 
where
    B: hyper::body::Body + 'static + Send,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to connect");
    let io = TokioIo::new(stream);

    let (sender, conn) = hyper::client::conn::http1::handshake::<_, B>(io)
        .await
        .expect("Failed to handshake");

    // Start the connection in the background
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection error: {:?}", err);
        }
    });

    sender
}

/// Convert a Response<Incoming> to a String
/// Panics if any error occurs
pub async fn response_to_string(response: Response<Incoming>) -> (u16, String) {
    let status = response.status().as_u16();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).expect("Invalid UTF-8");

    (status, body)
}
