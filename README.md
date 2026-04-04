# Qore Protocol

**Qore** is a high-performance Node.js framework for building real-time APIs that communicate over the **QUIC protocol** instead of HTTP. Define Express-style routes — data travels at UDP speed with built-in encryption.

The core is written in **Rust** (using Cloudflare's `quiche` library) and bridged to Node.js via **NAPI-RS**, giving you native performance with zero-copy memory.

## Features

- 🚀 **Express-style routing** — `app.route('/path', handler)` over QUIC
- ⚡ **Native QUIC client** — `client.send('/path', data)` with Promise responses
- 🔒 **Built-in TLS** — Auto-generated self-signed certs for development
- 🦀 **Rust core** — Zero-copy memory, async I/O via Tokio
- 📦 **Simple API** — 5 minutes to get started

---

## Prerequisites

- **Node.js** ≥ 18
- **Rust** (stable, edition 2021) — [rustup.rs](https://rustup.rs/)
- **NASM** — Required for BoringSSL on Windows ([nasm.us](https://www.nasm.us/))
- **C++ Build Tools** — Visual Studio Build Tools (Windows) or `build-essential` (Linux)

## Installation

```bash
git clone https://github.com/cesardarizaleta/qore.git
cd qore
npm install
npm run build
```

---

## Server Usage

Create a file `server.js`:

```javascript
const { Qore } = require('qore-protocol');

const app = new Qore();

// Define routes (just like Express!)
app.route('/echo', (req, res) => {
  console.log('Body:', req.body.toString());
  res.json({ echo: req.body.toString() });
});

app.route('/hello', (req, res) => {
  const data = req.json();           // Parse JSON body
  res.json({ message: `Hello, ${data?.name || 'World'}!` });
});

app.route('/users', (req, res) => {
  res.json({
    users: [
      { id: 1, name: 'Alice' },
      { id: 2, name: 'Bob' },
    ],
  });
});

// Lifecycle events
app.onConnection(({ peer }) => console.log(`Connected: ${peer}`));
app.onClosed(({ peer }) => console.log(`Disconnected: ${peer}`));

// Start listening (certs auto-generated for dev!)
app.listen(4433, () => {
  console.log('🚀 Qore server running on port 4433');
});
```

Run it:

```bash
node server.js
```

### With custom certificates (production)

```javascript
const app = new Qore({
  certPath: '/path/to/cert.crt',
  keyPath: '/path/to/cert.key',
});
```

---

## Client Usage

Create a file `client.js`:

```javascript
const { QoreClient } = require('qore-protocol');

async function main() {
  const client = new QoreClient();

  await client.connect('127.0.0.1', 4433);
  console.log('Connected!');

  // Send requests to routes (returns a Promise!)
  const echo = await client.send('/echo', { message: 'Hi!' });
  console.log('Echo:', echo);
  // → { echo: '{"message":"Hi!"}' }

  const hello = await client.send('/hello', { name: 'Qore' });
  console.log('Hello:', hello);
  // → { message: 'Hello, Qore!' }

  const users = await client.send('/users');
  console.log('Users:', users);
  // → { users: [{ id: 1, name: 'Alice' }, ...] }

  client.close();
}

main().catch(console.error);
```

---

## API Reference

### `Qore` (Server)

```typescript
const app = new Qore(options?: QoreOptions);
```

#### Options

| Property   | Type     | Description                    |
|------------|----------|--------------------------------|
| `certPath` | `string` | Path to TLS certificate file   |
| `keyPath`  | `string` | Path to TLS private key file   |

> If omitted, self-signed certificates are auto-generated for development.

#### Methods

| Method | Description |
|--------|-------------|
| `app.route(path, handler)` | Register a handler for a route |
| `app.onConnection(fn)` | Called when a peer connects |
| `app.onData(fn)` | Fallback for unmatched routes |
| `app.onClosed(fn)` | Called when a peer disconnects |
| `app.listen(port, callback?)` | Start listening on UDP port |

#### Handler signature

```typescript
app.route('/path', (req: QoreRequest, res: QoreResponse) => { ... });
```

**`QoreRequest`**
| Property   | Type       | Description                     |
|------------|------------|---------------------------------|
| `peer`     | `string`   | Remote address (`ip:port`)      |
| `streamId` | `number`   | QUIC stream ID                  |
| `route`    | `string`   | Matched route path              |
| `body`     | `Buffer`   | Raw request payload             |
| `json()`   | `() => any`| Parse body as JSON              |

**`QoreResponse`**
| Method      | Description                        |
|-------------|------------------------------------|
| `send(data)` | Send string, Buffer, or object    |
| `json(data)` | Send JSON response               |

---

### `QoreClient`

```typescript
const client = new QoreClient(timeout?: number);
```

| Parameter | Type     | Default | Description            |
|-----------|----------|---------|------------------------|
| `timeout` | `number` | 10000   | Request timeout in ms  |

#### Methods

| Method | Description |
|--------|-------------|
| `client.connect(host, port)` | Connect to a Qore server (Promise) |
| `client.send(route, data?)` | Send request and receive response (Promise) |
| `client.close()` | Close the connection |

#### Events

| Event        | Description                |
|--------------|----------------------------|
| `connection` | Fired when connected       |
| `closed`     | Fired when disconnected    |
| `data`       | Raw data on unknown stream |

---

## How It Works

Qore uses a simple binary frame protocol over QUIC streams:

```
┌──────────────────┬────────────────┬──────────────────┐
│ Route Length (2B) │ Route (UTF-8)  │ Payload (bytes)  │
│ uint16 BE        │ variable       │ rest of frame    │
└──────────────────┴────────────────┴──────────────────┘
```

Each `client.send()` opens a new QUIC bidirectional stream, sends the framed request, and waits for the framed response on the same stream.

### Architecture

```
  Node.js                         Node.js
┌──────────┐                   ┌────────────┐
│  Qore    │                   │ QoreClient │
│ .route() │                   │ .send()    │
│ .listen()│                   │ .connect() │
└────┬─────┘                   └─────┬──────┘
     │ Frame Protocol                │
     │ [routeLen][route][payload]    │
├────┴───────────────────────────────┴─────┤
│            Rust (NAPI-RS)                │
│  startServer()        connectToServer()  │
│         QUIC (quiche / BoringSSL)        │
│                UDP Socket                │
└──────────────────────────────────────────┘
```

## License

MIT
