// ─────────────────────────────────────────────────────────────
//  Qore Server — Example with Express-style routes
// ─────────────────────────────────────────────────────────────
const { Qore } = require('./dist/index.js');

const app = new Qore();

// ── Routes ───────────────────────────────────────────────────

app.route('/echo', (req, res) => {
  console.log(`[ECHO] from ${req.peer}:`, req.body.toString());
  res.json({
    echo: req.body.toString(),
    receivedBytes: req.body.length,
  });
});

app.route('/hello', (req, res) => {
  const data = req.json();
  const name = data?.name || 'World';
  res.json({ message: `Hello, ${name}!` });
});

app.route('/users', (req, res) => {
  res.json({
    users: [
      { id: 1, name: 'Alice' },
      { id: 2, name: 'Bob' },
      { id: 3, name: 'Charlie' },
    ],
  });
});

// ── Lifecycle events ─────────────────────────────────────────

app.onConnection(({ peer }) => {
  console.log(`[+] New connection from ${peer}`);
});

app.onClosed(({ peer }) => {
  console.log(`[-] Connection closed: ${peer}`);
});

// ── Start ────────────────────────────────────────────────────

app.listen(4433, () => {
  console.log('🚀 Qore server running on UDP port 4433');
  console.log('   Routes: /echo, /hello, /users');
});
