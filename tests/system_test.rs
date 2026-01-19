use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http1::handshake;
use hyper::Request;
use hyper_util::rt::TokioIo;
use rust_webserver::plugin::plugin_config::{PluginConfig, ProtocolEnum};
use rust_webserver::plugin_old::models::RequestInformation;
use std::env;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::fs;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn system_test() {
    // Locate the executable files
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let target_dir = current_dir.join("target").join("debug");

    let main_exe = if cfg!(windows) {
        target_dir.join("rust_webserver.exe")
    } else {
        target_dir.join("rust_webserver")
    };

    let test_plugin_exe = if cfg!(windows) {
        target_dir.join("test_plugin.exe")
    } else {
        target_dir.join("test_plugin")
    };

    // Ensure the executables exist
    assert!(
        main_exe.exists(),
        "Main executable not found at {:?}",
        main_exe
    );
    assert!(
        test_plugin_exe.exists(),
        "Test plugin executable not found at {:?}",
        test_plugin_exe
    );

    // Create temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let plugin_dir = temp_dir.path().join("plugins").join("test_plugin");
    fs::create_dir_all(&plugin_dir).await.unwrap();

    // Copy the main executable to temp directory
    let temp_main_exe = if cfg!(windows) {
        temp_dir.path().join("rust_webserver.exe")
    } else {
        temp_dir.path().join("rust_webserver")
    };
    fs::copy(&main_exe, &temp_main_exe).await.unwrap();

    // Copy the test plugin
    let dest_plugin_exe = if cfg!(windows) {
        plugin_dir.join("test_plugin.exe")
    } else {
        plugin_dir.join("test_plugin")
    };
    fs::copy(&test_plugin_exe, &dest_plugin_exe).await.unwrap();

    // Create plugin configuration
    let startup_cmd = if cfg!(windows) {
        ".\\test_plugin.exe"
    } else {
        "./test_plugin"
    };

    let config = PluginConfig {
        plugin_name: "test_plugin".to_string(),
        startup_command: startup_cmd.to_string(),
        protocol: ProtocolEnum::StdIoJson,
        max_request_timeout: 5000,
        max_startup_time: 5000,
        request_information: RequestInformation {
            request_methods: vec!["*".to_string()],
            hosts: vec!["*".to_string()],
            paths: vec!["*".to_string()],
        },
    };

    fs::write(
        &plugin_dir.join("plugin_config.json"),
        &serde_json::to_vec(&config).unwrap(),
    )
    .await
    .unwrap();

    println!("starting server..");
    // Start the webserver
    let mut server_process = Command::new(&temp_main_exe)
        .current_dir(temp_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Wait until the server is ready
    sleep(Duration::from_secs(2)).await;

    // Send HTTP request with hyper
    let test_body = "Hello, World!";

    // Connect to the server
    let stream = TcpStream::connect("127.0.0.1:80")
        .await
        .expect("Failed to connect");
    let io = TokioIo::new(stream);

    // HTTP/1 handshake
    let (mut sender, conn) = handshake(io).await.expect("Failed to handshake");

    // Start the connection in the background
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection error: {:?}", err);
        }
    });

    // Create the request
    let req = Request::builder()
        .method("POST")
        .uri("/test.json")
        .header("Host", "example.com")
        .header("Content-Type", "application/json")
        .body(Full::new(Bytes::from(test_body)))
        .expect("Failed to build request");

    // Send the request
    let response = sender
        .send_request(req)
        .await
        .expect("Failed to send request");

    // Read the response body
    let status = response.status();
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("Failed to read response body")
        .to_bytes();
    let response_body = String::from_utf8(body_bytes.to_vec()).expect("Invalid UTF-8");

    // Debug output
    println!("Response status: {}", status);
    println!("Response body: {}", response_body);

    // Check the status code
    assert_eq!(status, 200, "Expected status 200");

    // Check if the response body matches the request body
    assert_eq!(
        response_body, test_body,
        "Response body should match request body"
    );

    // Stop the server
    server_process
        .kill()
        .await
        .expect("Failed to kill server process");
}
