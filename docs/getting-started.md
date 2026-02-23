# Getting Started with Qore Protocol

This guide will help you set up and run your first Qore Protocol server.

## Prerequisites

Before you begin, ensure you have the following installed on your system:

1. **Node.js**: Version 18 or higher.
2. **Rust**: The latest stable version (Edition 2021). You can install it via [rustup](https://rustup.rs/).
3. **NASM**: Required for compiling BoringSSL (used by `quiche`) on Windows. Make sure it's added to your system's PATH.
4. **C++ Build Tools**: Visual Studio Build Tools on Windows, or `build-essential` on Linux.

## Installation

Clone the repository and install the dependencies:

```bash
git clone <repository-url>
cd qore-protocol
npm install
```

Build the native addon and compile the TypeScript code:

```bash
npm run build
```

This command will:
1. Compile the Rust code into a native Node.js addon (`.node` file) using `napi-rs`.
2. Compile the TypeScript wrapper into the `dist/` directory.

## Generating Certificates

QUIC requires TLS encryption. For local development, you can generate self-signed certificates using OpenSSL:

```bash
openssl req -x509 -newkey rsa:4096 -keyout cert.key -out cert.crt -days 365 -nodes -subj "/CN=localhost"
```

Place `cert.crt` and `cert.key` in the root of your project.

## Running the Server

Create a file named `server.js` (or use the provided `test-server.js`):

```javascript
const { QoreServer } = require('./dist/index.js');
const path = require('path');

async function main() {
  const server = new QoreServer({
    port: 4433,
    certPath: path.join(__dirname, 'cert.crt'),
    keyPath: path.join(__dirname, 'cert.key')
  });

  server.on('connection', (peer) => {
    console.log(`[Node.js] New connection from ${peer}`);
  });

  server.on('data', (peer, streamId, data) => {
    console.log(`[Node.js] Data from ${peer} on stream ${streamId}:`, data.toString());
    server.send(peer, streamId, Buffer.from(`Echo: ${data.toString()}`));
  });

  console.log('Starting Qore server on port 4433...');
  await server.start();
}

main();
```

Run the server:

```bash
node server.js
```

## Testing with the Rust Client

The project includes a standalone Rust client for testing. Open a new terminal and run:

```bash
cd qore-client
cargo run
```

You should see the client connect to the server, send a message, and receive the echoed response.
