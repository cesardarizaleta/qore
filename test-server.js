const { QoreServer } = require('./dist/index.js');
const path = require('path');

async function main() {
  const port = 4433;
  const certPath = path.join(__dirname, 'cert.crt');
  const keyPath = path.join(__dirname, 'cert.key');

  const server = new QoreServer({ port, certPath, keyPath });

  server.on('connection', (peer) => {
    console.log(`[Node.js] New connection from ${peer}`);
  });

  server.on('data', (peer, streamId, data) => {
    console.log(`[Node.js] Data from ${peer} on stream ${streamId}:`, data.toString());
    
    // Echo the data back to the client
    console.log(`[Node.js] Echoing data back to ${peer} on stream ${streamId}...`);
    server.send(peer, streamId, Buffer.from(`Echo: ${data.toString()}`));
  });

  server.on('closed', (peer) => {
    console.log(`[Node.js] Connection closed from ${peer}`);
  });

  server.on('error', (error) => {
    console.error(`[Node.js] Server error:`, error);
  });

  console.log(`Starting Qore server on port ${port}...`);
  try {
    await server.start();
  } catch (error) {
    console.error('Failed to start server:', error);
  }
}

main();
