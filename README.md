# Rust Webserver with Plugin Architecture

A modular, plugin-based HTTP/1.1 web server written in Rust. This server acts as a lightweight core that delegates request handling to external plugins, allowing for flexible and extensible web application development.

## Overview

This web server is designed with a minimal core that focuses on request routing and plugin management. The actual request processing logic is implemented in separate plugin applications that communicate with the core server through a well-defined protocol. This architecture provides several benefits:

- **Modularity**: Each plugin is an independent application with its own lifecycle
- **Language Agnostic**: Plugins can be written in any language that supports stdin/stdout communication
- **Isolation**: Plugins run in separate processes, providing fault isolation
- **Dynamic Routing**: Plugins can handle requests based on flexible host and path patterns
- **Hot Reloading**: Plugins can be restarted without shutting down the entire server

## Architecture

The server consists of the following main components:

- **Webserver Core**: HTTP/1.1 server built on top of Hyper that receives and processes HTTP requests
- **Plugin Manager**: Discovers, loads, starts, and manages the lifecycle of plugins
- **Plugin Communication**: Handles bidirectional communication with plugin processes via stdin/stdout
- **Request Router**: Routes incoming requests to the appropriate plugin based on matching patterns
- **IO Abstraction**: File system operations abstraction for configuration loading and testing

For a detailed description of all components, see [architecture/module description.md](architecture/module%20description.md).

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)

### Building the Server

```bash
cargo build --release
```

### Running the Server

```bash
cargo run --release -- --port 8080
```

Command-line options:
- `-p, --port <PORT>`: Port to bind the server to (default: 80)

### Directory Structure

The server expects the following directory structure:

```
rust_webserver/
├── plugins/              # Plugin directory (scanned recursively)
│   ├── plugin1/
│   │   ├── pluginConfig.json
│   │   └── plugin_executable
│   └── plugin2/
│       ├── pluginConfig.json
│       └── plugin_executable
├── error_logs/          # Error logs from plugins
└── websites/            # Optional: static file hosting (plugin-dependent)
```

The server will automatically create the `plugins/` and `error_logs/` directories if they don't exist.

## Plugin Configuration

Each plugin must have a `pluginConfig.json` file that describes its behavior and requirements.

### Plugin Config Schema

```json
{
  "pluginName": "MyPlugin",
  "startupCommand": "./my_plugin_executable",
  "protocol": "STD_IO_JSON",
  "maxRequestTimeout": 5000,
  "maxStartupTime": 5000,
  "requestInformation": {
    "requestMethods": ["GET", "POST"],
    "hosts": ["example.com", "*.example.com"],
    "paths": ["/api/*", "/users/:id"]
  }
}
```

### Configuration Fields

- **pluginName** (string, required): Unique identifier for the plugin
- **startupCommand** (string, required): Command to execute the plugin. Path is relative to the plugin config location
- **protocol** (enum, required): Communication protocol to use. Currently supported: `STD_IO_JSON`
- **maxRequestTimeout** (number, required): Maximum time in milliseconds to wait for a plugin response
- **maxStartupTime** (number, required): Maximum time in milliseconds to wait for plugin initialization
- **requestInformation** (object, required): Defines which requests this plugin can handle
  - **requestMethods** (array): HTTP methods the plugin accepts (e.g., `["GET", "POST"]`)
  - **hosts** (array): Host patterns the plugin matches. Supports wildcards (e.g., `["*"]`, `["*.example.com"]`)
  - **paths** (array): Path patterns the plugin matches. Supports wildcards and parameters (e.g., `["/api/*"]`, `["/users/:id"]`)

### Request Routing and Priority

When multiple plugins match a request, the server uses a specificity algorithm to select the most appropriate plugin:

1. More specific host patterns take precedence (e.g., `example.com` > `*.example.com` > `*`)
2. More specific path patterns take precedence (e.g., `/api/users` > `/api/*` > `/*`)
3. Exact matches score higher than wildcard matches

## Communication Protocol

Plugins communicate with the server core through stdin/stdout using a binary-framed JSON protocol. The protocol is fully documented in [architecture/communication protocol.md](architecture/communication%20protocol.md).

### Protocol Overview

All packages are sent with a 4-byte length prefix followed by the JSON payload:

```
[4 bytes: length] [JSON payload]
```

### Package Types

- **Handshake**: Initial protocol negotiation between server and plugin
- **Normal Request**: HTTP request forwarded from the server to the plugin
- **Normal Response**: HTTP response from the plugin back to the server
- **Error Notification**: Error reporting from plugin to server

For detailed protocol specifications, package formats, and examples, please refer to the [communication protocol documentation](architecture/communication%20protocol.md).

## Creating a Plugin

### Basic Plugin Requirements

1. **Accept JSON packages from stdin** with the length-prefixed format
2. **Respond to handshake request** during initialization
3. **Process HTTP requests** and respond with HTTP responses
4. **Handle the protocol** as specified in the communication protocol documentation

### Example Plugin Structure (Pseudocode)

```python
# Read package from stdin
def read_package():
    length_bytes = stdin.read(4)
    length = int.from_bytes(length_bytes, 'big')
    json_data = stdin.read(length)
    return json.loads(json_data)

# Send package to stdout
def send_package(package):
    json_data = json.dumps(package)
    length = len(json_data)
    stdout.write(length.to_bytes(4, 'big'))
    stdout.write(json_data.encode())
    stdout.flush()

# Main loop
while True:
    package = read_package()
    
    if package['packageType'] == 'handshakeRequest':
        send_package({
            'packageType': 'handshakeResponse',
            'content': {
                'responseCode': 0,
                'responseCodeText': 'Success'
            }
        })
    
    elif package['packageType'] == 'normalRequest':
        http_request = package['content']
        # Process the HTTP request
        response = process_request(http_request)
        send_package({
            'packageType': 'normalResponse',
            'content': response
        })
```

## Example Plugins

The repository includes example plugins:

- **FileLoaderPlugin**: Serves static files from a directory
- **EmailSubscribePlugin**: Handles email subscription requests

Refer to the `plugins/` directory for complete implementations.

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Logging

```bash
RUST_LOG=debug cargo run -- --port 8080
```

### Project Structure

```
src/
├── main.rs                          # Server entry point
├── lib.rs                           # Library exports
├── webserver/                       # HTTP server implementation
│   ├── webserver.rs                 # Webserver trait
│   └── http_1_server.rs             # HTTP/1.1 implementation
├── plugin/                          # Plugin management
│   ├── plugin_manager.rs            # Plugin lifecycle and routing
│   ├── plugin_entry.rs              # Plugin configuration
│   ├── plugin_config.rs             # Config parsing
│   └── running_plugin.rs            # Active plugin instance
├── plugin_communication/            # Plugin communication layer
│   ├── plugin_communicator.rs       # Communication interface
│   ├── package_handler.rs           # Binary protocol handler
│   ├── models.rs                    # Data structures
│   ├── protocols/                   # Protocol implementations
│   └── app_starter/                 # Process management
│       ├── plugin_starter.rs        # Plugin launcher interface
│       ├── default_plugin_starter.rs # Default launcher
│       └── default_program_controller.rs # Process controller
└── io/                              # File system abstraction
    ├── data_storage.rs              # Storage interface
    └── in_memory_storage.rs         # Test implementation
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Roadmap

- [ ] Support for additional communication protocols (WebSocket, gRPC)
- [ ] Plugin health monitoring and automatic restart
- [ ] HTTP/2 and HTTP/3 support
- [ ] Plugin sandboxing and resource limits
- [ ] Web-based plugin management interface
- [ ] Plugin marketplace and discovery system

## FAQ

**Q: Can plugins be written in languages other than Rust?**  
A: Yes! Plugins can be written in any language that supports reading from stdin and writing to stdout.

**Q: How do I debug a plugin?**  
A: You can use the test binaries in `src/bin/` to test plugins in isolation. Set `RUST_LOG=debug` for detailed logging.

**Q: What happens if a plugin crashes?**  
A: The server detects crashed plugins and logs errors to the `error_logs/` directory. The plugin can be restarted manually or automatically (future feature).

**Q: Can multiple plugins handle the same URL?**  
A: Yes, but the server will route the request to the plugin with the most specific matching pattern.

**Q: How do I update a plugin without restarting the server?**  
A: Currently, you need to manually restart the server. Hot reloading is planned for a future release.
