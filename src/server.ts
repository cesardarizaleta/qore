import { EventEmitter } from 'events';
import { startServer, QoreEvent, QoreServerHandle } from '../index.js';

export interface QoreServerOptions {
  port: number;
  certPath: string;
  keyPath: string;
}

export class QoreServer extends EventEmitter {
  private options: QoreServerOptions;
  private isRunning: boolean = false;
  private handle: QoreServerHandle | null = null;

  constructor(options: QoreServerOptions) {
    super();
    this.options = options;
  }

  public async start(): Promise<void> {
    if (this.isRunning) {
      throw new Error('QoreServer is already running');
    }

    this.isRunning = true;

    try {
      // startServer is a blocking call in Rust, so we await it.
      // It will only return if the server crashes or is explicitly stopped (not implemented yet).
      this.handle = await startServer(
        this.options.port,
        this.options.certPath,
        this.options.keyPath,
        (event: QoreEvent) => {
          if (!event) return;

          switch (event.eventType) {
            case 'connection':
              this.emit('connection', event.peer);
              break;
            case 'data':
              // event.data is a Uint8Array from Rust (Zero-Copy)
              // We wrap it in a Node.js Buffer for convenience without copying the underlying memory
              const buffer = event.data ? Buffer.from(event.data.buffer, event.data.byteOffset, event.data.byteLength) : Buffer.alloc(0);
              this.emit('data', event.peer, event.streamId, buffer);
              break;
            case 'closed':
              this.emit('closed', event.peer);
              break;
            default:
              this.emit('error', new Error(`Unknown event type: ${event.eventType}`));
          }
        }
      );
    } catch (error) {
      this.isRunning = false;
      this.emit('error', error);
      throw error;
    }
  }

  public send(peer: string, streamId: number, data: Uint8Array | Buffer) {
    if (!this.handle) {
      throw new Error('Server is not running');
    }
    
    // Convert Buffer to Uint8Array if necessary, though Buffer extends Uint8Array in Node.js
    const uint8Data = data instanceof Uint8Array ? data : new Uint8Array(data);
    this.handle.sendData(peer, streamId, uint8Data);
  }
}
