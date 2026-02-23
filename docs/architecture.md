# Architecture

Qore Protocol is designed for maximum performance and minimal overhead. It achieves this by combining the safety and speed of Rust with the asynchronous ecosystem of Node.js.

## Core Components

1. **Rust Engine (`src/lib.rs`)**:
   - Handles the heavy lifting of the QUIC protocol using Cloudflare's `quiche` library.
   - Manages the UDP socket and asynchronous I/O using the `tokio` runtime.
   - Maintains the state of active connections and streams.

2. **N-API Bridge (`napi-rs`)**:
   - Provides the interface between the compiled Rust code and the V8 JavaScript engine.
   - Exposes Rust functions to Node.js and allows Rust to call Node.js callbacks.

3. **Node.js Wrapper (`src/server.ts`)**:
   - Provides a user-friendly, object-oriented API (`QoreServer`).
   - Translates native events into standard Node.js `EventEmitter` events.

## Zero-Copy Memory Strategy

One of the primary bottlenecks in native Node.js addons is copying data between the native memory space (C++/Rust) and the V8 memory space (JavaScript). Qore Protocol minimizes this overhead using a Zero-Copy strategy.

When data is received over the network:
1. Rust reads the data into a `Vec<u8>`.
2. Instead of copying this data into a new V8 Buffer, `napi-rs` creates a `Uint8Array` that directly references the memory allocated by Rust.
3. Ownership of the memory is transferred to V8, which will garbage collect it when it's no longer needed.
4. The Node.js wrapper receives this `Uint8Array` and wraps it in a Node.js `Buffer` (which is a subclass of `Uint8Array`) without copying the underlying bytes.

## Threading Model

To prevent blocking the Node.js Event Loop, Qore Protocol uses a multi-threaded architecture:

1. **Node.js Main Thread**: Executes your JavaScript code and handles the `EventEmitter` callbacks.
2. **Tokio Background Thread**: When `startServer` is called, a new Tokio asynchronous task is spawned in the background. This task runs an infinite loop that:
   - Listens for incoming UDP packets.
   - Processes QUIC handshakes and stream data.
   - Listens for commands from Node.js (via an `mpsc` channel) to send data back to clients.
3. **Thread-Safe Callbacks**: When the Tokio thread needs to emit an event (e.g., new data received), it uses a `ThreadsafeFunction` provided by `napi-rs` to safely queue the callback execution onto the Node.js Main Thread.

## Bidirectional Communication

Sending data from Node.js to the Rust server is handled via an Actor pattern:

1. When the server starts, Rust creates a Multi-Producer, Single-Consumer (`mpsc`) channel.
2. The receiving end (`rx`) is held by the Tokio background loop.
3. The transmitting end (`tx`) is wrapped in a `QoreServerHandle` and returned to Node.js.
4. When `server.send()` is called in Node.js, it invokes a method on the `QoreServerHandle`, which sends a `ServerCommand::SendData` message through the channel.
5. The Tokio loop receives the command, looks up the connection by the peer's IP address, and enqueues the data onto the appropriate QUIC stream.
