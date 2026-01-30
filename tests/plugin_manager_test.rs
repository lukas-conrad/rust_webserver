use futures::FutureExt;
use rust_webserver::io::data_storage::DataStorage;
use rust_webserver::io::in_memory_storage::InMemoryDataStorage;
use rust_webserver::plugin::plugin_config::{PluginConfig, ProtocolEnum};
use rust_webserver::plugin::plugin_manager::PluginManager;
use rust_webserver::plugin::plugin_manager::RequestHandler;
use rust_webserver::plugin_communication::app_starter::plugin_starter::ProgramController;
use rust_webserver::plugin_communication::app_starter::test_plugin_starter::{
    TestPluginProgramController, TestPluginStarter,
};
use rust_webserver::plugin_communication::models::Package::{NormalRequest, NormalResponse};
use rust_webserver::plugin_communication::models::{
    HttpRequest, HttpResponse, NormalResponseContent, RequestInformation,
};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn plugin_manager_test() {
    let storage: Box<dyn DataStorage> = Box::new(InMemoryDataStorage::new());
    let startup_command = "./test_plugin.exe".to_string();
    let config = create_test_plugin_config(
        startup_command.clone(),
        "test/**/test_request".to_string(),
        "*.example.com".to_string(),
    );
    storage
        .store_data(
            serde_json::to_vec(&config).unwrap(),
            Path::new("plugins/test_plugin/pluginConfig.json"),
        )
        .await
        .unwrap();

    let mut plugin_starter = TestPluginStarter::new().await;
    plugin_starter.add_plugin(
        startup_command,
        Box::new(|| {
            async {
                Box::new(TestPluginProgramController::new(None).await) as Box<dyn ProgramController>
            }
            .boxed()
        }),
    );
    let mut plugin_manager = PluginManager::new(storage, Box::new(plugin_starter));

    plugin_manager
        .scan_plugins(Path::new("plugins"))
        .await
        .unwrap();

    assert_eq!(plugin_manager.plugin_entries.len(), 1);

    for entry in &plugin_manager.plugin_entries {
        plugin_manager.start_plugin(entry).await.unwrap();
    }

    assert_eq!(plugin_manager.plugins.read().await.len(), 1);

    let test_message = "test_message".to_string();
    let test_request = HttpRequest {
        request_method: "GET".to_string(),
        path: "test/hello/test_request".to_string(),
        host: "test.example.com".to_string(),
        headers: vec![],
        body: test_message.clone(),
    };

    let response = plugin_manager.route_request(test_request).await.unwrap();

    assert_eq!(response.body, test_message);
}

#[tokio::test]
async fn test_parallel_requests() {
    let storage: Box<dyn DataStorage> = Box::new(InMemoryDataStorage::new());
    let startup_command = "./test_plugin.exe".to_string();
    let config = create_test_plugin_config(
        startup_command.clone(),
        "test/**/test_request".to_string(),
        "*.example.com".to_string(),
    );
    storage
        .store_data(
            serde_json::to_vec(&config).unwrap(),
            Path::new("plugins/test_plugin/pluginConfig.json"),
        )
        .await
        .unwrap();

    let mut plugin_starter = TestPluginStarter::new().await;
    plugin_starter.add_plugin(
        startup_command,
        Box::new(|| {
            async {
                Box::new(
                    TestPluginProgramController::new(Some(Box::new(|package| {
                        async move {
                            if let NormalRequest(content) = package {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                return Some(NormalResponse(NormalResponseContent {
                                    package_id: content.package_id,
                                    http_response: HttpResponse {
                                        headers: vec![],
                                        status_code: 200,
                                        body: content.http_request.body.clone(),
                                    },
                                }));
                            }
                            return None;
                        }
                        .boxed()
                    })))
                    .await,
                ) as Box<dyn ProgramController>
            }
            .boxed()
        }),
    );
    let mut plugin_manager = PluginManager::new(storage, Box::new(plugin_starter));

    plugin_manager
        .scan_plugins(Path::new("plugins"))
        .await
        .unwrap();

    assert_eq!(plugin_manager.plugin_entries.len(), 1);

    for entry in &plugin_manager.plugin_entries {
        plugin_manager.start_plugin(entry).await.unwrap();
    }

    assert_eq!(plugin_manager.plugins.read().await.len(), 1);

    let plugin_manager = Arc::new(plugin_manager);
    let mut handles = Vec::new();

    let test_message = "test_message".to_string();

    let request_count = 50;
    let time = SystemTime::now();

    for _ in 0..request_count {
        let test_request = HttpRequest {
            request_method: "GET".to_string(),
            path: "test/hello/test_request".to_string(),
            host: "test.example.com".to_string(),
            headers: vec![],
            body: test_message.clone(),
        };

        let pm = Arc::clone(&plugin_manager);
        let handle = tokio::spawn(async move { pm.route_request(test_request).await });

        handles.push(handle);
    }

    futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|result| result.unwrap())
        .map(|result| result.unwrap())
        .for_each(|response| assert_eq!(response.body, test_message));

    let elapsed = time.elapsed().unwrap();
    println!(
        "{request_count} requests processed in {} ms",
        elapsed.as_millis()
    );
    assert!(elapsed < Duration::from_millis(200));
}

fn create_test_plugin_config(
    startup_command: String,
    request_path: String,
    request_host: String,
) -> PluginConfig {
    PluginConfig {
        plugin_name: "Test Plugin".to_string(),
        startup_command,
        protocol: ProtocolEnum::StdIoJson,
        max_request_timeout: 1000,
        max_startup_time: 1000,
        request_information: RequestInformation {
            request_methods: vec!["*".to_string()],
            hosts: vec![request_host],
            paths: vec![request_path],
        },
    }
}
