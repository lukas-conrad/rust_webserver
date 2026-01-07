# Module Description

## Communication

The communication module is used to communicate with the plugin via the communication Protocol.
It features sending a Package and setting a callback for incoming packages. Look up communication protocol for further
information.

## Plugin

This module features a Plugin Manager and a Plugin Struct. The Plugin Manager has functions to manage the plugins like
loading (from disk), starting (and initialize communication), stopping or restarting plugins and routing requests to
plugins. Each Plugin has a config
JSON that defines its properties. That's what is read from the File System. When started, a Plugin has a running
instance. This instance can be used to route requests to the plugins.

## IO

This module contains an abstraction of the File System to make setting up test scenarios easier. It contains methods for
loading and storing files.

## Controller

The Controller module contains the main class that uses the webserver to route requests to the plugin system and send
the results back to the webserver.

## Webserver

The Webserver is used to receive requests and send back responses.