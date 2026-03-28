/**
 * Helpers for WebSocket binary payloads (`ws` RawData → Buffer, safe send).
 */

import { WebSocket } from 'ws';

import type { RawData } from 'ws';

export function bufferFromRaw(data: RawData): Buffer {
  if (Buffer.isBuffer(data)) return data;
  if (Array.isArray(data)) return Buffer.concat(data);
  return Buffer.from(data);
}

export function sendBinaryQuiet(ws: WebSocket, payload: Buffer, onFail?: () => void): void {
  if (ws.readyState !== WebSocket.OPEN) return;
  try {
    ws.send(payload, { binary: true });
  } catch {
    onFail?.();
  }
}
