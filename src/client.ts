import { EventEmitter } from 'events';
import { connectToServer, QoreEvent, QoreClientHandle } from '../index.js';

// ── Types ────────────────────────────────────────────────────────────

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (reason: any) => void;
  chunks: Buffer[];
  timeout: ReturnType<typeof setTimeout>;
}

// ── Frame helpers ────────────────────────────────────────────────────
// Frame format: [2B route-length] [4B payload-length] [route UTF-8] [payload]

function buildFrame(route: string, payload: Buffer): Buffer {
  const routeBuf = Buffer.from(route, 'utf-8');
  const header = Buffer.alloc(6);
  header.writeUInt16BE(routeBuf.length, 0);
  header.writeUInt32BE(payload.length, 2);
  return Buffer.concat([header, routeBuf, payload]);
}

// ── QoreClient ───────────────────────────────────────────────────────

export class QoreClient extends EventEmitter {
  private handle: QoreClientHandle | null = null;
  private connected: boolean = false;
  private nextStreamId: number = 0; // client-initiated bidi: 0, 4, 8, ...
  private pending: Map<number, PendingRequest> = new Map();
  private requestTimeout: number;

  /**
   * @param timeout  Request timeout in ms (default 10 000)
   */
  constructor(timeout: number = 10_000) {
    super();
    this.requestTimeout = timeout;
  }

  // ── Connect ────────────────────────────────────────

  /**
   * Open a QUIC connection to a Qore server.
   * Resolves when the handshake is complete.
   */
  async connect(host: string, port: number): Promise<void> {
    return new Promise<void>(async (resolve, reject) => {
      try {
        this.handle = await connectToServer(host, port, (err: Error | null, event: QoreEvent) => {
          if (err) { this.emit('error', err); return; }
          if (!event) return;

          switch (event.eventType) {
            case 'connection':
              this.connected = true;
              this.emit('connection');
              resolve();
              break;

            case 'data': {
              const buffer = event.data
                ? Buffer.from(event.data.buffer, event.data.byteOffset, event.data.byteLength)
                : Buffer.alloc(0);

              const streamId = event.streamId || 0;
              const req = this.pending.get(streamId);

              if (req) {
                // Accumulate response chunks
                req.chunks.push(buffer);
                const full = Buffer.concat(req.chunks);

                // Try to parse the response frame
                if (full.length >= 6) {
                  const routeLen = full.readUInt16BE(0);
                  const payloadLen = full.readUInt32BE(2);
                  const totalFrameLen = 6 + routeLen + payloadLen;

                  if (full.length >= totalFrameLen) {
                    const payload = full.subarray(6 + routeLen, totalFrameLen);
                    clearTimeout(req.timeout);
                    this.pending.delete(streamId);
                    // Try JSON, fallback to raw buffer
                    try {
                      req.resolve(JSON.parse(payload.toString()));
                    } catch {
                      req.resolve(payload);
                    }
                  }
                }
              } else {
                this.emit('data', streamId, buffer);
              }
              break;
            }

            case 'closed':
              this.connected = false;
              this.emit('closed');
              // Reject all pending requests
              for (const [id, req] of this.pending) {
                clearTimeout(req.timeout);
                req.reject(new Error('Connection closed'));
                this.pending.delete(id);
              }
              break;
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  // ── Send request ───────────────────────────────────

  /**
   * Send a request to a route and receive the response.
   *
   * @param route  Route path (e.g. `/echo`)
   * @param data   Optional payload (object → JSON, string, Buffer)
   * @returns      Parsed response (JSON object or Buffer)
   */
  async send(route: string, data?: any): Promise<any> {
    if (!this.handle || !this.connected) {
      throw new Error('Not connected. Call connect() first.');
    }

    const streamId = this.nextStreamId;
    this.nextStreamId += 4; // QUIC client-initiated bidi streams

    // Serialize payload
    let payload: Buffer;
    if (data === undefined || data === null) {
      payload = Buffer.alloc(0);
    } else if (Buffer.isBuffer(data)) {
      payload = data;
    } else if (typeof data === 'object') {
      payload = Buffer.from(JSON.stringify(data));
    } else {
      payload = Buffer.from(String(data));
    }

    const frame = buildFrame(route, payload);

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (this.pending.has(streamId)) {
          this.pending.delete(streamId);
          reject(new Error(`Request to ${route} timed out after ${this.requestTimeout}ms`));
        }
      }, this.requestTimeout);

      this.pending.set(streamId, { resolve, reject, chunks: [], timeout });

      this.handle!.sendOnStream(streamId, frame, true).catch((err: Error) => {
        clearTimeout(timeout);
        this.pending.delete(streamId);
        reject(err);
      });
    });
  }

  // ── Close ──────────────────────────────────────────

  /** Close the connection gracefully */
  close(): void {
    this.connected = false;
    for (const [id, req] of this.pending) {
      clearTimeout(req.timeout);
      req.reject(new Error('Client closed'));
      this.pending.delete(id);
    }
    this.handle = null;
  }
}

export default QoreClient;
