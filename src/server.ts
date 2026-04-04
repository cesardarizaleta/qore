import { EventEmitter } from 'events';
import fs from 'fs';
import path from 'path';
import selfsigned from 'selfsigned';
import { startServer, QoreEvent, QoreServerHandle } from '../index.js';

// ── Types ────────────────────────────────────────────────────────────

export interface QoreOptions {
  certPath?: string;
  keyPath?: string;
}

export interface QoreRequest {
  /** Remote peer address (ip:port) */
  peer: string;
  /** QUIC stream ID */
  streamId: number;
  /** Matched route path */
  route: string;
  /** Raw body as Buffer */
  body: Buffer;
  /** Parse body as JSON (returns null on failure) */
  json: () => any;
}

export interface QoreResponse {
  /** Send raw data (string, Buffer, or object → auto JSON) */
  send: (data: string | Buffer | object) => void;
  /** Send JSON response */
  json: (data: any) => void;
}

export type RouteHandler = (req: QoreRequest, res: QoreResponse) => void;

// ── Frame helpers ────────────────────────────────────────────────────
// Frame format: [2B route-length BE][route UTF-8][payload]

function parseFrame(raw: Buffer): { route: string; payload: Buffer } {
  if (raw.length < 2) return { route: '/', payload: raw };
  const routeLen = raw.readUInt16BE(0);
  if (raw.length < 2 + routeLen) return { route: '/', payload: raw };
  const route = raw.subarray(2, 2 + routeLen).toString('utf-8');
  const payload = raw.subarray(2 + routeLen);
  return { route, payload };
}

function buildFrame(route: string, payload: Buffer): Buffer {
  const routeBuf = Buffer.from(route, 'utf-8');
  const header = Buffer.alloc(2);
  header.writeUInt16BE(routeBuf.length, 0);
  return Buffer.concat([header, routeBuf, payload]);
}

// ── Qore Server ──────────────────────────────────────────────────────

export class Qore extends EventEmitter {
  private options: QoreOptions;
  private isRunning: boolean = false;
  private handle: QoreServerHandle | null = null;
  private routes: Map<string, RouteHandler> = new Map();

  private connectionHandler?: (info: { peer: string }) => void;
  private closedHandler?: (info: { peer: string }) => void;
  private fallbackHandler?: (req: QoreRequest, res: QoreResponse) => void;

  constructor(options: QoreOptions = {}) {
    super();
    this.options = options;
  }

  // ── Route registration (Express-style) ────────────

  /** Register a handler for a specific route path */
  public route(routePath: string, handler: RouteHandler): this {
    this.routes.set(routePath, handler);
    return this;
  }

  // ── Lifecycle callbacks ────────────────────────────

  /** Called when a new QUIC connection is established */
  public onConnection(handler: (info: { peer: string }) => void): this {
    this.connectionHandler = handler;
    return this;
  }

  /** Fallback handler for data without a matching route */
  public onData(handler: (req: QoreRequest, res: QoreResponse) => void): this {
    this.fallbackHandler = handler;
    return this;
  }

  /** Called when a connection is closed */
  public onClosed(handler: (info: { peer: string }) => void): this {
    this.closedHandler = handler;
    return this;
  }

  // ── Start listening ────────────────────────────────

  public async listen(port: number, callback?: () => void): Promise<void> {
    if (this.isRunning) throw new Error('Qore server is already running');

    let { certPath, keyPath } = this.options;

    // Auto-generate self-signed certs for development
    if (!certPath || !keyPath || !fs.existsSync(certPath) || !fs.existsSync(keyPath)) {
      console.warn('⚠️  No SSL certificates found. Generating self-signed certs for development...');
      const attrs = [{ name: 'commonName', value: 'localhost' }];
      const pems = selfsigned.generate(attrs, { days: 30 });

      const certsDir = path.join(process.cwd(), '.qore_certs');
      if (!fs.existsSync(certsDir)) fs.mkdirSync(certsDir, { recursive: true });

      certPath = path.join(certsDir, 'cert.crt');
      keyPath = path.join(certsDir, 'cert.key');
      fs.writeFileSync(certPath, pems.cert);
      fs.writeFileSync(keyPath, pems.private);
    }

    this.isRunning = true;

    try {
      this.handle = await startServer(port, certPath, keyPath, (err: Error | null, event: QoreEvent) => {
        if (err) { this.emit('error', err); return; }
        if (!event) return;

        switch (event.eventType) {
          case 'connection':
            if (this.connectionHandler) this.connectionHandler({ peer: event.peer });
            this.emit('connection', event.peer);
            break;

          case 'data': {
            const rawBuffer = event.data
              ? Buffer.from(event.data.buffer, event.data.byteOffset, event.data.byteLength)
              : Buffer.alloc(0);

            const { route, payload } = parseFrame(rawBuffer);

            const req: QoreRequest = {
              peer: event.peer,
              streamId: event.streamId || 0,
              route,
              body: payload,
              json: () => {
                try { return JSON.parse(payload.toString()); }
                catch { return null; }
              },
            };

            const res: QoreResponse = {
              send: (data: string | Buffer | object) => {
                if (!this.handle) return;
                let output: Buffer;
                if (Buffer.isBuffer(data)) output = data;
                else if (data instanceof Uint8Array) output = Buffer.from(data);
                else if (typeof data === 'object') output = Buffer.from(JSON.stringify(data));
                else output = Buffer.from(String(data));
                // Response frame: same route header + payload
                const frame = buildFrame(route, output);
                this.handle.sendData(event.peer, event.streamId || 0, frame);
              },
              json: (data: any) => {
                if (!this.handle) return;
                const output = Buffer.from(JSON.stringify(data));
                const frame = buildFrame(route, output);
                this.handle.sendData(event.peer, event.streamId || 0, frame);
              },
            };

            // Route matching
            const handler = this.routes.get(route);
            if (handler) {
              handler(req, res);
            } else if (this.fallbackHandler) {
              this.fallbackHandler(req, res);
            }
            this.emit('data', event.peer, event.streamId, rawBuffer);
            break;
          }

          case 'closed':
            if (this.closedHandler) this.closedHandler({ peer: event.peer });
            this.emit('closed', event.peer);
            break;

          default:
            this.emit('error', new Error(`Unknown event type: ${event.eventType}`));
        }
      });

      if (callback) callback();
    } catch (error) {
      this.isRunning = false;
      this.emit('error', error);
      throw error;
    }
  }
}
