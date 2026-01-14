use rust_webserver::io::data_storage::FSBinding;
use rust_webserver::plugin::plugin_config::{PluginConfig, ProtocolEnum};
use rust_webserver::plugin::plugin_entry::PluginEntry;
use rust_webserver::plugin_communication::app_starter::default_app_starter::DefaultAppStarter;
use rust_webserver::plugin_communication::app_starter::plugin_starter::{AppController, PluginStarter};
use rust_webserver::plugin_old::models::RequestInformation;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Helper function to create a passthrough FSBinding for tests
fn create_passthrough_fs_binding() -> Arc<dyn FSBinding> {
    struct PassthroughFSBinding;

    impl FSBinding for PassthroughFSBinding {
        fn translate_to_fs(&self, path: &Box<Path>) -> Result<Box<Path>, rust_webserver::io::data_storage::FileSystemError> {
            Ok(path.clone())
        }
    }

    Arc::new(PassthroughFSBinding)
}

/// Helper function to create a test PluginEntry
fn create_test_plugin_entry(startup_command: String) -> PluginEntry {
    let config = PluginConfig {
        plugin_name: "test_plugin".to_string(),
        startup_command,
        protocol: ProtocolEnum::StdIoJson,
        max_request_timeout: 5000,
        max_startup_time: 3000,
        request_information: RequestInformation {
            request_methods: vec![],
            hosts: vec![],
            paths: vec![],
        },
    };

    // Use current directory as the plugin path
    let current_dir = std::env::current_dir().unwrap();
    let plugin_path = current_dir.join("dummy_plugin_path");

    PluginEntry::new(config, plugin_path.into_boxed_path())
}

/// Get the path to the test_dummy_app binary
fn get_dummy_app_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove the test executable name

    // Handle different build configurations
    if path.ends_with("deps") {
        path.pop();
    }

    #[cfg(target_os = "windows")]
    path.push("test_dummy_app.exe");

    #[cfg(not(target_os = "windows"))]
    path.push("test_dummy_app");

    path
}

#[tokio::test]
async fn test_start_app_success() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --sleep 100", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let result = starter.start_app(&entry).await;
    assert!(result.is_ok(), "Failed to start app: {:?}", result.err());

    let mut controller = result.unwrap();
    assert!(controller.is_running(), "App should be running after start");

    // Clean up
    let _ = controller.shutdown().await;
}

#[tokio::test]
async fn test_controller_stdin_stdout() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --echo", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Get stdin and stdout
    let mut stdin = controller.get_stdin().unwrap();
    let stdout = controller.get_stdout().unwrap();
    let mut reader = BufReader::new(stdout);

    // Write to stdin
    let test_message = "Hello from test\n";
    stdin.write_all(test_message.as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    // Read from stdout
    let mut response = String::new();
    reader.read_line(&mut response).await.unwrap();

    assert_eq!(response.trim(), "Hello from test");

    // Clean up
    drop(stdin);
    let _ = controller.shutdown().await;
}

#[tokio::test]
async fn test_controller_stderr() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let error_message = "Test-error-message";
    let command = format!("{} --stderr-message {}", dummy_path.display(), error_message);
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Get stderr
    let stderr = controller.get_stderr().unwrap();
    let mut reader = BufReader::new(stderr);

    // Read from stderr
    let mut response = String::new();
    reader.read_line(&mut response).await.unwrap();

    assert_eq!(response.trim(), error_message);

    // Wait for process to complete
    let _ = controller.wait().await;
}

#[tokio::test]
async fn test_controller_exit_code() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let exit_code = 42;
    let command = format!("{} --exit-code {}", dummy_path.display(), exit_code);
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Wait for the process to complete
    let status = controller.wait().await.unwrap();

    assert_eq!(status.code(), Some(exit_code));
}

#[tokio::test]
async fn test_controller_is_running() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --sleep 2000", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Should be running
    assert!(controller.is_running(), "Process should be running");

    // Shutdown the process
    controller.shutdown().await.unwrap();

    // Give it a moment to stop
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Should not be running anymore
    assert!(!controller.is_running(), "Process should have stopped");
}

#[tokio::test]
async fn test_controller_shutdown() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --infinite-loop", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    assert!(controller.is_running(), "Process should be running");

    // Shutdown the process
    let result = controller.shutdown().await;
    assert!(result.is_ok(), "Shutdown should succeed");

    // Wait a bit for the process to be killed
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Process should not be running
    assert!(!controller.is_running(), "Process should have been killed");
}

#[tokio::test]
async fn test_controller_wait() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --sleep 100 --exit-code 5", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Wait for process to complete
    let status = controller.wait().await.unwrap();

    // Should have exited with code 5
    assert_eq!(status.code(), Some(5));

    // Should not be running anymore
    assert!(!controller.is_running(), "Process should have completed");
}

#[tokio::test]
async fn test_controller_get_stdin_twice_fails() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --echo", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // First call should succeed
    let stdin1 = controller.get_stdin();
    assert!(stdin1.is_ok(), "First get_stdin should succeed");

    // Second call should fail because stdin was already taken
    let stdin2 = controller.get_stdin();
    assert!(stdin2.is_err(), "Second get_stdin should fail");

    // Clean up
    drop(stdin1);
    let _ = controller.shutdown().await;
}

#[tokio::test]
async fn test_controller_get_stdout_twice_fails() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --echo", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // First call should succeed
    let stdout1 = controller.get_stdout();
    assert!(stdout1.is_ok(), "First get_stdout should succeed");

    // Second call should fail because stdout was already taken
    let stdout2 = controller.get_stdout();
    assert!(stdout2.is_err(), "Second get_stdout should fail");

    // Clean up
    drop(stdout1);
    let _ = controller.shutdown().await;
}

#[tokio::test]
async fn test_controller_get_stderr_twice_fails() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!("{} --stderr-message \"test\"", dummy_path.display());
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // First call should succeed
    let stderr1 = controller.get_stderr();
    assert!(stderr1.is_ok(), "First get_stderr should succeed");

    // Second call should fail because stderr was already taken
    let stderr2 = controller.get_stderr();
    assert!(stderr2.is_err(), "Second get_stderr should fail");

    // Clean up
    drop(stderr1);
    let _ = controller.wait().await;
}

#[tokio::test]
async fn test_multiple_commands_combined() {
    let fs_binding = create_passthrough_fs_binding();
    let starter = DefaultAppStarter::new(fs_binding);

    let dummy_path = get_dummy_app_path();
    let command = format!(
        "{} --stderr-message Starting --sleep 200 --exit-code 7",
        dummy_path.display()
    );
    let entry = create_test_plugin_entry(command);

    let mut controller = starter.start_app(&entry).await.unwrap();

    // Read stderr message
    let stderr = controller.get_stderr().unwrap();
    let mut reader = BufReader::new(stderr);
    let mut stderr_msg = String::new();
    reader.read_line(&mut stderr_msg).await.unwrap();
    assert_eq!(stderr_msg.trim(), "Starting");

    // Should be running during sleep
    assert!(controller.is_running(), "Should be running during sleep");

    // Wait for completion
    let status = controller.wait().await.unwrap();
    assert_eq!(status.code(), Some(7));
}

