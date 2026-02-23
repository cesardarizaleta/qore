# Ejemplo: Echo server (Node.js) y cliente (Rust)

Este ejemplo muestra cómo ejecutar un servidor `QoreServer` en Node.js que devuelve (echo) los datos recibidos y cómo usar el cliente Rust incluido (`qore-client`) para probar la comunicación.

Requisitos previos

- Haber compilado el addon nativo:

```bash
npm install
npm run build
```

- Generar certificados TLS (desarrollo):

```bash
openssl req -x509 -newkey rsa:4096 -keyout cert.key -out cert.crt -days 365 -nodes -subj "/CN=localhost"
```

Asegúrate de que `cert.crt` y `cert.key` estén en la raíz del proyecto (no los subas al repo).

Servidor Node.js (echo)

Crea `server.js` o usa `test-server.js`. Ejemplo mínimo:

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
    console.log(`[Node] New connection from ${peer}`);
  });

  server.on('data', (peer, streamId, data) => {
    console.log(`[Node] Received (${peer} stream ${streamId}):`, data.toString());
    // Echo back
    server.send(peer, streamId, Buffer.from(`Echo: ${data.toString()}`));
  });

  server.on('closed', (peer) => console.log(`[Node] Closed: ${peer}`));
  server.on('error', (err) => console.error('[Node] Error:', err));

  console.log('Starting Qore server on port 4433...');
  await server.start();
}

main().catch(console.error);
```

Cliente Rust (incluido)

En la carpeta `qore-client/` ya hay un cliente de prueba. Para ejecutarlo:

```bash
cd qore-client
cargo run
```

Comportamiento esperado

1. En una terminal, inicia el servidor Node.js:

```bash
node test-server.js
```

Salida esperada (servidor):

```
Starting Qore server on port 4433...
Qore Server listening on 0.0.0.0:4433
[Node] New connection from 127.0.0.1:XXXXX
[Node] Received (127.0.0.1:XXXXX stream 4): Hello from Qore Client!
[Node] Echoing data back to 127.0.0.1:XXXXX on stream 4...
```

2. En otra terminal, ejecuta el cliente Rust. Salida esperada (cliente):

```
Sending ... bytes to 127.0.0.1:4433
Received ... bytes from 127.0.0.1:4433
QUIC connection established successfully!
Sent data on stream 4
Received data from server on stream 4: Echo: Hello from Qore Client!
```

Notas y troubleshooting

- Si la conexión falla en Windows, instala `nasm` y las Visual C++ Build Tools.
- Si ves errores `CryptoFail` durante handshake, espera unos ciclos; son habituales en handshakes incompletos.
- Para producción, gestiona certificados con un almacén seguro (no usar certificados auto-firmados) y configura CI para precompilar binarios.

Siguientes pasos sugeridos

- Añadir un ejemplo JS que actúe como cliente (si necesitas interoperar con otro runtime JS).
- Automatizar este ejemplo como test E2E en GitHub Actions.
