mod test_utils;

use bytes::Bytes;
use http_body_util::Full;
use hyper::Request;
use rust_webserver::plugin::plugin_config::{PluginConfig, ProtocolEnum};
use rust_webserver::plugin_old::models::RequestInformation;
use std::env;
use std::net::TcpListener;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;
use test_utils::{check_server_running, print_stdio, response_to_string, setup_sender};
use tokio::fs;
use tokio::time::{sleep, Duration};

/// Find a free port by binding to port 0 and getting the assigned port
fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port 0");
    listener
        .local_addr()
        .expect("Failed to get local addr")
        .port()
}

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
        max_request_timeout: 1000,
        max_startup_time: 1000,
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

    // Find a free port
    let port = find_free_port();
    println!("Using port {} for test", port);

    // Start the webserver with debug logging
    let mut server_process = Command::new(&temp_main_exe)
        .current_dir(temp_dir.path())
        .env("RUST_LOG", "debug")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start server");

    // Capture stdout and stderr
    let stdout = server_process.stdout.take().expect("Failed to get stdout");
    let stderr = server_process.stderr.take().expect("Failed to get stderr");

    // Spawn tasks to read stdout and stderr and print them in real-time
    let stdout_task = print_stdio(stdout, "[SERVER]".to_string());
    let stderr_task = print_stdio(stderr, "[SERVER ERR]".to_string());

    // Check if the server process is still running
    sleep(Duration::from_millis(500)).await;
    check_server_running(&mut server_process);

    // Wait for the server to start
    sleep(Duration::from_secs(2)).await;

    // Send HTTP request with hyper
    let test_body = "Hello, World!";

    // Setup HTTP connection
    let mut sender = setup_sender::<Full<Bytes>>(port).await;

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
    let (status, response_body) = response_to_string(response).await;

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
        .expect("Failed to kill server process");

    server_process
        .wait()
        .expect("Failed to wait for server process");

    // Abort the logging tasks
    drop(stdout_task);
    drop(stderr_task);
}
