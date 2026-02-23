# API Reference

## `QoreServer`

The `QoreServer` class is the main entry point for the Qore Protocol in Node.js. It extends the standard Node.js `EventEmitter`.

### Constructor

```typescript
constructor(options: QoreServerOptions)
```

#### `QoreServerOptions`

- `port` (number): The UDP port the server will listen on.
- `certPath` (string): The absolute path to the TLS certificate file (`.crt` or `.pem`).
- `keyPath` (string): The absolute path to the TLS private key file (`.key`).

### Methods

#### `start(): Promise<void>`

Starts the QUIC server. This method binds the UDP socket and begins listening for incoming connections. It returns a Promise that resolves when the server is successfully started, or rejects if an error occurs (e.g., port already in use).

#### `send(peer: string, streamId: number, data: Uint8Array | Buffer): void`

Sends data to a specific peer on a specific stream.

- `peer` (string): The IP address and port of the connected client (e.g., `"127.0.0.1:54321"`). This value is provided in the `connection` and `data` events.
- `streamId` (number): The ID of the QUIC stream to send data on.
- `data` (Uint8Array | Buffer): The binary data to send.

### Events

The `QoreServer` instance emits the following events:

#### `connection`

Emitted when a new QUIC connection is established.

- `peer` (string): The IP address and port of the client.

```javascript
server.on('connection', (peer) => { ... });
```

#### `data`

Emitted when data is received from a client on a specific stream.

- `peer` (string): The IP address and port of the client.
- `streamId` (number): The ID of the stream the data was received on.
- `data` (Buffer): The received data. This is a Zero-Copy buffer backed by the memory allocated in Rust.

```javascript
server.on('data', (peer, streamId, data) => { ... });
```

#### `closed`

Emitted when a connection with a client is closed.

- `peer` (string): The IP address and port of the client.

```javascript
server.on('closed', (peer) => { ... });
```

#### `error`

Emitted when an error occurs in the server.

- `error` (Error): The error object.

```javascript
server.on('error', (error) => { ... });
```
