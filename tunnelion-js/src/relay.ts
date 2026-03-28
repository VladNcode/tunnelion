/**
 * Tunnel relay: TCP listeners per tunnel, WebSocket control plane, frame multiplexing.
 */

import net from 'node:net';

import { WebSocket, WebSocketServer } from 'ws';

import { secretsEqual, tryParseAuthPreface } from './auth-preface.js';
import { SERVER_AUTH_OK, WS_MAX_PAYLOAD } from './constants.js';
import { encodeClose, encodeData, encodeOpen, tryParseFrame } from './frames.js';
import { waitFirstMessage } from './helpers.js';
import { parseHostPort } from './host-port.js';
import { bufferFromRaw, sendBinaryQuiet } from './ws-raw.js';

import type { RelayConfig } from './config.js';
import type { RawData } from 'ws';

// --- types -----------------------------------------------------------------------------------

export type ActiveSessionRef = { current: ControlSession | null };

// --- control session (one WebSocket) ---------------------------------------------------------

export class ControlSession {
  constructor(
    readonly ws: WebSocket,
    readonly streams = new Map<number, net.Socket>(),
    public nextStreamId = 1,
  ) {}

  bindPublicSocket(pub: net.Socket, streamId: number, tunnelName: string): void {
    pub.setNoDelay(true);
    this.streams.set(streamId, pub);
    try {
      this.ws.send(encodeOpen(streamId, tunnelName), { binary: true });
    } catch {
      pub.destroy();
      this.streams.delete(streamId);
      return;
    }
    pub.on('data', (chunk: Buffer) => {
      sendBinaryQuiet(this.ws, encodeData(streamId, chunk), () => pub.destroy());
    });
    pub.on('end', () => {
      this.streams.delete(streamId);
      sendBinaryQuiet(this.ws, encodeClose(streamId));
    });
    pub.on('error', () => {
      this.streams.delete(streamId);
      sendBinaryQuiet(this.ws, encodeClose(streamId));
    });
  }
}

async function runControlSession(ws: WebSocket, expectedSecret: Buffer, active: ActiveSessionRef): Promise<void> {
  let first: Buffer;
  try {
    first = await waitFirstMessage(ws);
  } catch {
    ws.close();
    return;
  }

  const got = tryParseAuthPreface(first);
  if (!got || !secretsEqual(got, expectedSecret)) {
    ws.close();
    return;
  }

  try {
    ws.send(SERVER_AUTH_OK, { binary: true });
  } catch {
    ws.close();
    return;
  }

  const sess = new ControlSession(ws);
  active.current = sess;

  ws.on('message', (data: RawData) => {
    const frame = tryParseFrame(bufferFromRaw(data));
    if (!frame) return;
    if (frame.op === 'data') {
      const pub = sess.streams.get(frame.streamId);
      if (pub && !pub.destroyed) pub.write(frame.payload);
    } else if (frame.op === 'close') {
      const pub = sess.streams.get(frame.streamId);
      if (pub) {
        pub.destroy();
        sess.streams.delete(frame.streamId);
      }
    }
  });

  await new Promise<void>(resolve => {
    ws.once('close', () => {
      for (const pub of sess.streams.values()) pub.destroy();
      sess.streams.clear();
      if (active.current === sess) active.current = null;
      resolve();
    });
  });
}

// --- public API ------------------------------------------------------------------------------

export function startRelay(cfg: RelayConfig, secretUtf8: string): void {
  const secretBuf = Buffer.from(secretUtf8, 'utf8');
  const active: ActiveSessionRef = { current: null };

  for (const name of Object.keys(cfg.tunnels)) {
    const entry = cfg.tunnels[name];
    const srv = net.createServer(pub => {
      const sess = active.current;
      if (!sess || sess.ws.readyState !== WebSocket.OPEN) {
        pub.destroy();
        return;
      }
      const id = sess.nextStreamId++;
      sess.bindPublicSocket(pub, id, name);
    });
    const { host, port } = parseHostPort(entry.listen);
    srv.listen(port, host, () => {
      console.error(`tunnelion-js relay: tunnel ${JSON.stringify(name)} listening on ${entry.listen}`);
    });
    srv.on('error', err => {
      console.error(`tunnelion-js relay: tunnel listener ${name}:`, err);
      process.exit(1);
    });
  }

  const { host, port } = cfg.control;
  const wss = new WebSocketServer({ host, port, maxPayload: WS_MAX_PAYLOAD });

  let chain = Promise.resolve();
  wss.on('connection', ws => {
    chain = chain
      .then(() => runControlSession(ws, secretBuf, active))
      .catch(e => console.error('tunnelion-js relay: control session:', e));
  });

  wss.on('listening', () => {
    console.error(
      `tunnelion-js relay: WebSocket control ws://${host}:${port} (${cfg.control.host}:${cfg.control.port})`,
    );
  });
  wss.on('error', err => {
    console.error('tunnelion-js relay: WebSocket server:', err);
    process.exit(1);
  });
}
