/**
 * Tunnel client: WebSocket to relay, auth preface, then multiplex streams to configured upstreams.
 */

import net from 'node:net';

import { encodeAuthPreface } from './auth-preface.js';
import { FAIL_INITIAL_MS, FAIL_MAX_MS, SESSION_END_MS } from './constants.js';
import { encodeClose, encodeData, tryParseFrame } from './frames.js';
import { buildNameToUpstream, connectWs, sleep, waitAuthOk } from './helpers.js';
import { parseHostPort } from './host-port.js';
import { bufferFromRaw, sendBinaryQuiet } from './ws-raw.js';

import type { ClientConfig } from './config.js';
import type { ParsedFrame } from './frames.js';
import type { RawData, WebSocket } from 'ws';

function formatUpstreamError(upstream: string, err: unknown): string {
  const code = err && typeof err === 'object' && 'code' in err ? (err as NodeJS.ErrnoException).code : undefined;
  if (code === 'ECONNREFUSED') {
    return `tunnelion-js client: upstream ${upstream}: connection refused (nothing listening)`;
  }
  if (code === 'ETIMEDOUT') {
    return `tunnelion-js client: upstream ${upstream}: connection timed out`;
  }
  if (code === 'ENOTFOUND' || code === 'EAI_AGAIN') {
    return `tunnelion-js client: upstream ${upstream}: ${code} (name resolution)`;
  }
  if (code === 'EHOSTUNREACH' || code === 'ENETUNREACH') {
    return `tunnelion-js client: upstream ${upstream}: ${code}`;
  }
  if (code === 'EPIPE' || code === 'ECONNRESET') {
    return `tunnelion-js client: upstream ${upstream}: ${code} (connection closed)`;
  }
  const msg = err instanceof Error ? err.message : String(err);
  return `tunnelion-js client: upstream ${upstream}: ${msg}`;
}

// --- session ---------------------------------------------------------------------------------

class TunnelSession {
  constructor(
    readonly ws: WebSocket,
    readonly streams = new Map<number, net.Socket>(),
    readonly nameToUpstream: Map<string, string>,
  ) {}

  dispatch(payload: Buffer): void {
    const frame = tryParseFrame(payload);
    if (!frame) return;
    switch (frame.op) {
      case 'open':
        this.onOpen(frame);
        break;
      case 'data':
        this.onData(frame);
        break;
      case 'close':
        this.onClose(frame);
        break;
    }
  }

  private onOpen(frame: Extract<ParsedFrame, { op: 'open' }>): void {
    const upstream = this.nameToUpstream.get(frame.tunnelName);
    if (!upstream) {
      console.error(`tunnelion-js client: unknown tunnel ${JSON.stringify(frame.tunnelName)}`);
      sendBinaryQuiet(this.ws, encodeClose(frame.streamId));
      return;
    }
    const { host, port } = parseHostPort(upstream);
    const local = net.connect({ host, port });
    this.streams.set(frame.streamId, local);

    local.once('connect', () => {
      console.error(
        `tunnelion-js client: tunnel ${JSON.stringify(frame.tunnelName)} → ${upstream} (stream ${frame.streamId})`,
      );
    });
    local.once('error', err => {
      console.error(formatUpstreamError(upstream, err));
      this.streams.delete(frame.streamId);
      sendBinaryQuiet(this.ws, encodeClose(frame.streamId));
      local.destroy();
    });
    local.on('data', (chunk: Buffer) => {
      sendBinaryQuiet(this.ws, encodeData(frame.streamId, chunk), () => local.destroy());
    });
    local.on('end', () => {
      this.streams.delete(frame.streamId);
      sendBinaryQuiet(this.ws, encodeClose(frame.streamId));
    });
  }

  private onData(frame: Extract<ParsedFrame, { op: 'data' }>): void {
    const s = this.streams.get(frame.streamId);
    if (s && !s.destroyed) s.write(frame.payload);
  }

  private onClose(frame: Extract<ParsedFrame, { op: 'close' }>): void {
    const s = this.streams.get(frame.streamId);
    if (s) {
      s.destroy();
      this.streams.delete(frame.streamId);
    }
  }
}

// --- one session + reconnect loop ------------------------------------------------------------

async function runOneSession(cfg: ClientConfig, secret: string): Promise<void> {
  const ws = await connectWs(cfg.relay.url);
  ws.send(encodeAuthPreface(secret), { binary: true });
  try {
    await waitAuthOk(ws);
  } catch (e) {
    ws.close();
    throw e;
  }

  console.error(`tunnelion-js client: connected ${cfg.relay.url} (auth ok)`);

  const session = new TunnelSession(ws, new Map(), buildNameToUpstream(cfg));
  ws.on('message', (data: RawData) => {
    session.dispatch(bufferFromRaw(data));
  });

  await new Promise<void>(resolve => {
    ws.once('close', resolve);
  });
}

// --- public API ------------------------------------------------------------------------------

export async function runClientForever(cfg: ClientConfig, secret: string): Promise<never> {
  let failBackoff = FAIL_INITIAL_MS;
  for (;;) {
    try {
      await runOneSession(cfg, secret);
      console.error('tunnelion-js client: session ended; reconnect in', SESSION_END_MS, 'ms');
      await sleep(SESSION_END_MS);
      failBackoff = FAIL_INITIAL_MS;
    } catch (e) {
      console.error('tunnelion-js client: session failed:', e);
      await sleep(failBackoff);
      failBackoff = Math.min(failBackoff * 2, FAIL_MAX_MS);
    }
  }
}
