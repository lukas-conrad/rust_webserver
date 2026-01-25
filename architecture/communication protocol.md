# Webserver

---

## General description:

The Webserver should be built modular. The core should only handle requests and delegate them to plugins. The Plugins should be launched by the Core and they should communicate to each other by using the standart IO.

## Procedure

At Startup, the Webserver should scan the “/plugins” directory (and sub directorys) for every “pluginConfig.json”. The startup command from the plugin config is executed locally to the plugin config to start the plugins. The Handshake is initiated from the Core side to the Plugin. After the handshake is done, the Server routes requests to the different plugins like described by the plugins json. If the plugin encounters a error, it sends the error to the Core. If the Core wants to restart the plugin, it first sends a stop packet to the plugin and waits for its termination. The plugin config contains a timeout that is used for any request that requires a response (also for the handshake). Every package that is sent from the plugin to the core is validated. If it is invalid (wrong format, etc.), the error is reported.

## JSON Protocol

Every Package is sent with the following Layout:

| Bytes      | Content           |
|------------|-------------------|
| 0 - 3      | Length of Package |
| 4 - length | JSON package      |

### Handshake

**Core to Plugin**:

The Core provides the Plugin with the chosen protocol from the protocol list of the plugin config

```json
{
  "packageType": "handshakeRequest",
  "content": {
    "protocol": "json"
  }
}
```

**Plugin to Core**:

The Plugin responds with a response code. Response codes could be:

- Code 0: Success
- Code 1: Plugin error (custom error message in the response code text)
- Code 2: Protocol not supported

```json
{
  "packageType": "handshakeResponse",
  "content": {
    "responseCode": 1,
    "responseCodeText": "Plugin initialization error"
  }
}
```

### Request

This is the normal package for a webrequest routed to the plugin. The host should be provided in every case. If its HTTP 1, then the Host is just found in the headers but with HTTP2, it is called authority.

The request is async. Every request to the plugin has a unique id. The response from the plugin also has this id. This makes it easy for the core to track to which request the plugin response is mapped. If the Plugin exceeds the maximum timeout for the request, the request is terminated and an error is returned back to the sender.

**Core to Plugin**:

```json
{
  "packageType": "normalRequest",
  "content": {
    "packageId": 12345,
    "httpRequest": {
      "requestMethod": "GET",
      "path": "home/helloWorld.html",
      "host": "api.server.de",
      "headers": [
        {
          "key": "Accept",
          "value": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        }
      ],
      "body": "request body"
    }
  }
}
```

**Plugin to Core**:

```json
{
  "packageType": "normalResponse",
  "content": {
    "packageId": 12345,
    "httpResponse": {
      "headers": [
        {
          "key": "Content-Encoding",
          "value": "gzip"
        }
      ],
      "body": "response body"
    }
  }
}
```

### Shutdown

This package is used to stop the plugin (when stopping the server) or restarting the plugin.

**Core to Plugin**:

```json
{
  "packageType": "shutdownRequest",
  "content": {

  }
}
```

### Error

This package is used to communicate errors to the core. The core can then handle the error (maybe report it / save it into a file). The policy field then specifies if the plugin can be restarted or not.

Possible policies:

- restart - Restarts the plugin
- stop - Stops the plugin, does not restart it
- report - Leaves the plugin running, just reports the error

**Client to Core**:

```json
{
  "packageType": "error",
  "content": {
    "errorCode": 15902,
    "errorDescription": "Fatal error, plugin is corrupt",
    "policy": "restart"
  }
}
```

### Logging

This package is used by plugins to send log messages to the core. The core can handle these logs according to its configuration (e.g., save to file, forward to a logging system, display in console).

Log levels:
- debug - Detailed information, typically of interest only when diagnosing problems
- info - Confirmation that things are working as expected
- warning - An indication that something unexpected happened, or may happen in the near future
- error - An error occurred, but the plugin can still function
- critical - A critical error that may lead to plugin malfunction

**Plugin to Core**:

```json
{
  "packageType": "log",
  "content": {
    "level": "info",
    "message": "Successfully processed request"
  }
}
```

### Plugin Config JSON:

Every Plugin needs a plugin JSON so the core knows hot to communicate and can route the requests. The request information is used by the core to route packages. If multiple Plugins have the same requestInformation, the more specific one is picked. For example “path/path-to-website/*” is picked over “path/*”. Starts are allowed in every field for generalization.

The timeout is in milliseconds

The plugin config looks like this:

```json
{
  "pluginName": "InternalBusinessHandler420",
  "startupCommand": "java -jar bussinessHandler.jar",
  "protocols": ["json"],
  "maxRequestTimeout": 1000,
  "maxStartupTime": 1000,
  "requestInformation": {
    "requestMethods": ["*"],
    "hosts": ["api.server.de", "business.server.*"],
    "paths": ["api/*", "api/**/business/*"]
  }
}
```

## Error Handling

Every error that occurs by plugins is saved with date and time. The error file should look like the following JSON:

Error Types are:

- ValidationError
- PluginError - Returned Error code (from plugin error package) is written into the error details

```json
{
  "pluginName": "Internal Business logic",
  "errorType": "ValidationError",
  "errorName": "Invalid response from Plugin",
  "errorDetails": "Plugin returned a invalid json: { packageN87q24ijo }"
}
```