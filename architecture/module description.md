# Component Description

## Plugin Component

The plugin component provides the core plugin management infrastructure of the system.

### Plugin Manager
The `PluginManager` is responsible for the complete lifecycle management of plugins:
- **Plugin Discovery**: Scans the file system for plugin configurations (pluginConfig.json files)
- **Plugin Loading**: Reads and parses plugin configurations from disk
- **Plugin Lifecycle**: Starts, stops, and restarts plugins as needed
- **Request Routing**: Routes incoming HTTP requests to appropriate plugins based on host and path matching patterns
- **Plugin Selection**: Uses a specificity-based algorithm to determine which plugin should handle a request when multiple plugins match

### Plugin Entry
The `PluginEntry` represents a plugin configuration loaded from disk. It contains:
- Plugin metadata and configuration settings
- Compiled regex patterns for efficient host and path matching
- Request routing rules and specificity calculation logic
- Path to the plugin executable and working directory

### Running Plugin
The `RunningPlugin` represents an active, running plugin instance with:
- An established communication channel to the plugin process
- Protocol handling for package exchange
- Handshake initialization and verification
- Timeout management for requests and startup
- Package sending and response handling with filters

### Plugin Config
Contains the data structures for parsing and validating plugin configuration files, including:
- Plugin name and version
- Supported protocols
- Request matching patterns (hosts and paths)
- Timeout settings
- Startup command configuration

## Plugin Communication Component

This component handles all communication aspects between the core server and plugin processes.

### Plugin Communicator
The `PluginCommunicator` interface and its implementation `JsonCommunicator` provide:
- **Bidirectional Communication**: Send packages to plugins and receive responses
- **Listener System**: Register callbacks for incoming packages from plugins
- **Response Filtering**: Filter and route responses to the appropriate waiting request handlers
- **Asynchronous Processing**: Non-blocking communication using async/await patterns

### Package Handler
Low-level package transmission handling:
- **Binary Protocol**: Implements the length-prefixed binary protocol (4 bytes length + payload)
- **Stream Management**: Manages stdin/stdout streams for plugin communication
- **Continuous Reading**: Spawns background tasks to continuously read from plugin output
- **Package Serialization**: Handles conversion between bytes and structured data

### Protocol System
Defines and implements communication protocols:
- **Protocol Trait**: Interface for different communication protocol implementations
- **StdIO JSON Protocol**: Default protocol using JSON over standard input/output
- **Protocol Lifecycle**: Handles protocol initialization and cleanup
- **Protocol Selection**: Negotiates protocol during handshake phase

### Models
Data structures for communication:
- **Package Types**: HandshakeRequest/Response, NormalRequest/Response
- **HTTP Models**: HttpRequest and HttpResponse structures
- **Content Types**: Structured content for different package types
- **Serialization**: Serde-based JSON serialization/deserialization

## App Starter Component

Manages the launching and control of plugin processes.

### Plugin Starter
The `PluginStarter` trait defines the interface for starting plugin applications:
- **Process Creation**: Launches plugin executables as separate processes
- **Working Directory**: Sets the correct working directory for plugin execution
- **Stream Setup**: Configures stdin/stdout/stderr for communication
- **Error Handling**: Handles startup failures and reports errors

### Default Plugin Starter
Default implementation of the `PluginStarter` trait:
- **Command Execution**: Uses tokio::process::Command to spawn processes
- **Environment Setup**: Configures environment variables and working directories
- **Stream Piping**: Establishes piped streams for communication

### Program Controller
The `ProgramController` trait provides control over running plugin processes:
- **Stream Access**: Provides access to stdin/stdout/stderr of the running process
- **Status Monitoring**: Checks if the process is still running
- **Graceful Shutdown**: Sends termination signals to processes
- **Exit Status**: Waits for process termination and retrieves exit codes

### Default Program Controller
Default implementation managing a single child process:
- **Stream Extraction**: Extracts and provides access to process streams
- **Process Monitoring**: Uses try_wait() to check process status without blocking
- **Forced Termination**: Implements process killing when needed
- **Resource Cleanup**: Ensures proper cleanup of process resources

## IO Component

Provides abstraction over file system operations to facilitate testing and modularity.

### Data Storage
The `DataStorage` trait defines a unified interface for file operations:
- **Load Operations**: Read file contents as byte arrays
- **Store Operations**: Write byte arrays to files
- **Delete Operations**: Remove files from storage
- **Directory Listing**: Recursively or non-recursively list directory contents
- **Error Handling**: Unified error types for all file system operations

### FSDataStorage
File system-based implementation of `DataStorage`:
- **Path Translation**: Maps logical paths to physical file system paths
- **Async Operations**: Uses tokio::fs for non-blocking file I/O
- **Recursive Traversal**: Supports recursive directory scanning
- **Error Mapping**: Converts I/O errors to domain-specific error types

### In-Memory Storage
Test implementation that stores data in memory:
- **Mock File System**: HashMap-based file storage for testing
- **Deterministic Behavior**: Controllable behavior for test scenarios
- **No Side Effects**: Doesn't touch the actual file system
- **Fast Execution**: Eliminates I/O latency in tests

## Webserver Component

Handles incoming HTTP requests and delegates them to the plugin system.

### Webserver Trait
Defines the interface for web server implementations:
- **Request Listener**: Registers callbacks for incoming HTTP requests
- **Server Lifecycle**: Start and stop operations for the server
- **Port Binding**: Configuration of network interfaces and ports

### HTTP/1.1 Server
Implementation of the Webserver trait using Hyper:
- **Request Reception**: Listens for and parses incoming HTTP/1.1 requests
- **Request Delegation**: Forwards requests to the registered callback (typically the PluginManager)
- **Response Handling**: Sends HTTP responses back to clients
- **Connection Management**: Handles keep-alive and connection pooling
- **Error Responses**: Generates appropriate error responses for failures
