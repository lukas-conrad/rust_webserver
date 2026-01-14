This is where I put my mental Nodes.

Here, I want to run a complete tracer path through all my modules, mentally from start to finish.

App Start:
- Scanning files for plugin_configs, load them and provide a list of plugin entries
- Start all plugins from the entries → List of running plugins
- Start Webserver, set request listener

Request arrives:
- Extract parameters from request
- Route to plugin manager
- Find Plugin for params
- Send package to plugin
- Receive response from plugin
- Send response to Webserver
- Send response to client

Plugin communication:
- Use Plugin protocol to start running plugin
- Send packages via protocol to plugin
- Receive packages via protocol from plugin

Advanced communication:
- Send packages directly
- Optional Listener for receiving an answer

Walkthrough:
- call send packet with packet, filter (to filter from all incoming packages) and timeout
- Wait for method to return the response package