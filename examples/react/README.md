# Qore Browser Example

This folder contains a minimal static example that connects to the WebSocket proxy (`examples/proxy/proxy-server.js`) and shows how a browser can send/receive data through the proxy to the native `QoreServer`.

Instructions:

1. From the workspace root, start the proxy (it starts the native `QoreServer` and the WS proxy):

```bash
npm install ws
node examples/proxy/proxy-server.js
```

2. Open `examples/react/index.html` in a browser (or serve the folder with a static server) and click `Connect`, then `Send Test`.

Notes:
- This example uses plain WebSocket messages encoded as JSON with base64 payloads. The proxy relays `data`, `connect`, and `peers` events.
- For a production React app you'd bundle `src/client.ts` (or compile to JS) and reuse the `QoreClient` abstraction instead of the inline code.
