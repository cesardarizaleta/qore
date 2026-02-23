import { EventEmitter } from 'events'

type Incoming = { type: string; peer?: string; streamId?: number; data?: string; peers?: string[] }

export class QoreClient extends EventEmitter {
  ws: any
  url: string

  constructor(url: string) {
    super()
    this.url = url
    this.ws = null
  }

  async connect(): Promise<void> {
    let WebSocketImpl: any = (globalThis as any).WebSocket
    if (!WebSocketImpl) {
      try {
        WebSocketImpl = require('ws')
      } catch (e) {
        throw new Error("No WebSocket available. In Node.js install 'ws' or provide a global WebSocket implementation")
      }
    }

    this.ws = new WebSocketImpl(this.url)

    this.ws.onopen = () => this.emit('open')
    this.ws.onclose = () => this.emit('close')
    this.ws.onerror = (err: any) => this.emit('error', err)
    this.ws.onmessage = (ev: any) => {
      const raw = ev.data ?? ev
      const text = typeof raw === 'string' ? raw : raw.toString()
      try {
        const msg: Incoming = JSON.parse(text)
        if (msg.type === 'data' && msg.peer && msg.data) {
          const bin = (typeof Buffer !== 'undefined' && Buffer.from)
            ? Buffer.from(msg.data, 'base64')
            : Uint8Array.from(atob(msg.data), c => c.charCodeAt(0))
          this.emit('data', msg.peer, msg.streamId ?? 0, bin)
        } else if (msg.type === 'peers') {
          this.emit('peers', msg.peers)
        } else if (msg.type === 'connect') {
          this.emit('connect', msg.peer)
        } else {
          this.emit('message', msg)
        }
      } catch (e) {
        this.emit('message', text)
      }
    }

    return new Promise((resolve, reject) => {
      this.once('open', () => resolve())
      this.once('error', (err: any) => reject(err))
    })
  }

  send(peer: string, streamId: number, data: Uint8Array | Buffer) {
    if (!this.ws) throw new Error('Not connected')
    const openState = this.ws.OPEN ?? 1
    if (this.ws.readyState !== openState) throw new Error('WebSocket not open')
    const base64 = (typeof Buffer !== 'undefined' && Buffer.from)
      ? Buffer.from(data).toString('base64')
      : btoa(String.fromCharCode(...Array.from(data)))
    this.ws.send(JSON.stringify({ type: 'send', peer, streamId, data: base64 }))
  }

  close() {
    if (this.ws) this.ws.close()
  }
}

export default QoreClient
