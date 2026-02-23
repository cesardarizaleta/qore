# Qore Protocol

**Qore** is a high-performance data transport library for Node.js, built as a Native Addon using Rust. It provides ultra-fast and robust communication using the QUIC protocol (via Cloudflare's `quiche` library) and prioritizes Zero-Copy memory strategies to minimize latency and CPU usage.

## Features

- **QUIC Protocol**: Built on top of UDP, providing multiplexed streams, low latency, and built-in encryption.
- **Native Performance**: Core logic is written in Rust (Edition 2021) and bridged to Node.js via N-API (`napi-rs`).
- **Zero-Copy Memory**: Passes data between the Rust engine and the Node.js Event Loop using shared memory buffers (`Uint8Array`), avoiding expensive memory duplication.
- **Asynchronous I/O**: Powered by Tokio for efficient background processing without blocking the Node.js main thread.
- **Developer Experience**: Provides a clean, object-oriented TypeScript API (`QoreServer` extending `EventEmitter`).

## Documentation

For detailed documentation, please refer to the `docs/` directory:

- [Getting Started](docs/getting-started.md)
- [API Reference](docs/api.md)
- [Architecture](docs/architecture.md)

## Quick Start

### Installation

Currently, Qore Protocol is built from source. Ensure you have the following prerequisites:
- Node.js (v18+)
- Rust (Edition 2021)
- NASM (for compiling BoringSSL on Windows)

```bash
npm install
npm run build
```

### Basic Usage

```typescript
import { QoreServer } from 'qore-protocol';

const server = new QoreServer({
  port: 4433,
  certPath: 'cert.crt',
  keyPath: 'cert.key'
});

server.on('connection', (peer) => {
  console.log(`New connection from: ${peer}`);
});

server.on('data', (peer, streamId, data) => {
  console.log(`Received data on stream ${streamId}:`, data.toString());
  
  // Echo data back to the client
  server.send(peer, streamId, Buffer.from(`Echo: ${data.toString()}`));
});

server.start().catch(console.error);
```

## License

MIT

