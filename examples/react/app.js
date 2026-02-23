const log = (msg) => {
  const p = document.getElementById('log')
  p.textContent = `${p.textContent}\n${msg}`
}

let ws = null
let peerId = null

document.getElementById('connect').addEventListener('click', () => {
  ws = new WebSocket('ws://localhost:8080')
  ws.onopen = () => log('connected')
  ws.onclose = () => log('closed')
  ws.onerror = (e) => log('error:'+e)
  ws.onmessage = (ev) => {
    try {
      const msg = JSON.parse(ev.data)
      if (msg.type === 'connect') {
        peerId = msg.peer
        log('server connect: '+ peerId)
      } else if (msg.type === 'data') {
        const bytes = atob(msg.data)
        log(`data from ${msg.peer} (${msg.streamId}): ${bytes}`)
      } else if (msg.type === 'peers') {
        log('peers: '+ JSON.stringify(msg.peers))
      } else {
        log('message: '+ JSON.stringify(msg))
      }
    } catch (e) {
      log('non-json message: '+ ev.data)
    }
  }
})

document.getElementById('send').addEventListener('click', () => {
  if (!ws || ws.readyState !== WebSocket.OPEN) return log('not connected')
  if (!peerId) return log('no peer yet')
  const text = 'hello-from-browser'
  const b64 = btoa(text)
  ws.send(JSON.stringify({ type: 'send', peer: peerId, streamId: 0, data: b64 }))
  log('sent test payload')
})
