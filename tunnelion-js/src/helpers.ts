/**
 * Shared helpers (sleep, WebSocket bootstrap, env, JSON shape checks).
 */

import { WebSocket } from 'ws';

import { SERVER_AUTH_OK, WS_MAX_PAYLOAD } from './constants.js';
import { bufferFromRaw } from './ws-raw.js';

import type { RawData } from 'ws';

// --- JSON / config ----------------------------------------------------------------------------

export function isObject(x: unknown): x is Record<string, unknown> {
  return x !== null && typeof x === 'object' && !Array.isArray(x);
}

/** Map tunnel name → upstream from a client-style config shape. */
export function buildNameToUpstream(cfg: { tunnels: Record<string, { upstream: string }> }): Map<string, string> {
  const m = new Map<string, string>();
  for (const name of Object.keys(cfg.tunnels)) {
    m.set(name, cfg.tunnels[name].upstream);
  }
  return m;
}

// --- process / CLI ----------------------------------------------------------------------------

export function requireSecret(): string {
  const s = process.env.TUNNELION_SECRET;
  if (s === undefined || s === '') {
    throw new Error('missing environment variable TUNNELION_SECRET');
  }
  return s;
}

// --- async / time ------------------------------------------------------------------------------

export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => {
    setTimeout(resolve, ms);
  });
}

// --- WebSocket ---------------------------------------------------------------------------------

export function connectWs(url: string, maxPayload: number = WS_MAX_PAYLOAD): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url, { maxPayload });
    ws.once('open', () => {
      ws.off('error', reject);
      resolve(ws);
    });
    ws.once('error', reject);
  });
}

/** Wait for first binary/text frame and return payload as Buffer. */
export function waitFirstMessage(ws: WebSocket): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const onErr = (e: Error) => {
      cleanup();
      reject(e);
    };
    const onClose = () => {
      cleanup();
      reject(new Error('WebSocket closed before auth'));
    };
    const cleanup = () => {
      ws.off('error', onErr);
      ws.off('close', onClose);
    };
    ws.once('message', (data: RawData) => {
      cleanup();
      resolve(bufferFromRaw(data));
    });
    ws.once('error', onErr);
    ws.once('close', onClose);
  });
}

/** After sending auth preface, wait for server `OK` frame. */
export function waitAuthOk(ws: WebSocket): Promise<void> {
  return new Promise((resolve, reject) => {
    const onErr = (e: Error) => {
      cleanup();
      reject(e);
    };
    const onClose = () => {
      cleanup();
      reject(new Error('WebSocket closed during auth'));
    };
    const cleanup = () => {
      ws.off('error', onErr);
      ws.off('close', onClose);
    };
    ws.once('message', (data: RawData) => {
      cleanup();
      const b = bufferFromRaw(data);
      if (b.equals(SERVER_AUTH_OK)) resolve();
      else reject(new Error('auth rejected'));
    });
    ws.once('error', onErr);
    ws.once('close', onClose);
  });
}
