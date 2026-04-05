// ─────────────────────────────────────────────────────────────
//  Qore-QUIC Client — Example connecting to a Qore-QUIC server
// ─────────────────────────────────────────────────────────────
const { QoreClient } = require('./dist/index.js');

async function main() {
  const client = new QoreClient();

  console.log('⏳ Connecting to Qore-QUIC server at 127.0.0.1:4433...');
  await client.connect('127.0.0.1', 4433);
  console.log('✅ Connected!\n');

  // ── Send requests to different routes ──────────────────

  // 1. Echo
  console.log('→ Sending to /echo ...');
  const echoResponse = await client.send('/echo', { message: 'Hello from Qore-QUIC Client!' });
  console.log('← Echo response:', echoResponse);
  console.log();

  // 2. Hello with a name
  console.log('→ Sending to /hello ...');
  const helloResponse = await client.send('/hello', { name: 'Qore-QUIC' });
  console.log('← Hello response:', helloResponse);
  console.log();

  // 3. Get users
  console.log('→ Sending to /users ...');
  const usersResponse = await client.send('/users');
  console.log('← Users response:', usersResponse);
  console.log();

  // ── Clean up ───────────────────────────────────────────

  client.close();
  console.log('🔌 Client disconnected.');
  process.exit(0);
}

main().catch((err) => {
  console.error('❌ Error:', err);
  process.exit(1);
});
