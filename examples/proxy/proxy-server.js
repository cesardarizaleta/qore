#!/usr/bin/env node
const path = require('path')

let WebSocketServer
try {
  WebSocketServer = require('ws').WebSocketServer
} catch (e) {
  console.error("Missing dependency 'ws'. Install with: npm install ws")
  process.exit(1)
}

const { QoreServer } = require(path.resolve(__dirname, '../../dist/index.js'))
const fs = require('fs')

async function main() {
  const certPath = path.join(__dirname, '../../cert.crt')
  const keyPath = path.join(__dirname, '../../cert.key')
  // If certs are missing, generate a temporary self-signed cert for dev
  if (!fs.existsSync(certPath) || !fs.existsSync(keyPath)) {
    console.log('TLS cert or key not found. Generating temporary self-signed certificate for development...')
    let selfsigned
    try {
      selfsigned = require('selfsigned')
    } catch (e) {
      console.error("Missing dependency 'selfsigned'. Install with: npm install selfsigned")
      process.exit(1)
    }
    const attrs = [{ name: 'commonName', value: 'localhost' }]
    const pems = selfsigned.generate(attrs, { days: 365 })
    fs.writeFileSync(certPath, pems.cert)
    fs.writeFileSync(keyPath, pems.private)
    console.log('Wrote temporary certs to', certPath, keyPath)
  }

  const server = new QoreServer({ port: 4433, certPath, keyPath })
  try {
    await server.start()
  } catch (err) {
    console.error('Failed to start QoreServer:', err)
    process.exit(1)
  }

  return server
}

main().then((server) => {
  global.__QORE_SERVER__ = server
}).catch((err) => {
  console.error('Startup error', err)
  process.exit(1)
})

const wss = new WebSocketServer({ port: 8080 })
console.log('Proxy: WebSocket server listening on ws://localhost:8080')

const clients = new Set()

wss.on('connection', (ws) => {
  console.log('Proxy: browser connected')
  clients.add(ws)

  ws.on('message', (msg) => {
    let obj
    try { obj = JSON.parse(msg.toString()) } catch (e) { return }
    if (obj.type === 'send' && obj.peer && obj.data) {
      const buf = Buffer.from(obj.data, 'base64')
      try {
        const s = getServer()
        if (s) s.send(obj.peer, obj.streamId || 0, buf)
      } catch (err) {
        console.error('Proxy: server.send failed', err)
      }
    }
  })

  ws.on('close', () => clients.delete(ws))
})

// When server becomes available, wire events
const getServer = () => global.__QORE_SERVER__

const onConnect = (peer) => {
  const payload = JSON.stringify({ type: 'connect', peer })
  for (const c of clients) c.send(payload)
}
const onData = (peer, streamId, data) => {
  const payload = JSON.stringify({ type: 'data', peer, streamId, data: data.toString('base64') })
  for (const c of clients) c.send(payload)
}

// Poll until server is set and then attach handlers
const attachInterval = setInterval(() => {
  const server = getServer()
  if (!server) return
  clearInterval(attachInterval)
  server.on('connect', onConnect)
  server.on('data', onData)
  console.log('Proxy started.')
}, 200)
