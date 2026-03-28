/**
 * Tunnel frames: one WebSocket binary message = one frame (after auth OK).
 */

import { OP_CLOSE, OP_DATA, OP_OPEN } from './constants.js';

// --- types -------------------------------------------------------------------------------------

export type ParsedFrame =
  | { op: 'open'; streamId: number; tunnelName: string }
  | { op: 'data'; streamId: number; payload: Buffer }
  | { op: 'close'; streamId: number };

// --- encode (client ↔ relay) -------------------------------------------------------------------

export function encodeOpen(streamId: number, tunnelName: string): Buffer {
  const name = Buffer.from(tunnelName, 'utf8');
  if (name.length > 65535) throw new Error('tunnel name too long');
  const out = Buffer.alloc(1 + 4 + 2 + name.length);
  out.writeUInt8(OP_OPEN, 0);
  out.writeUInt32BE(streamId, 1);
  out.writeUInt16BE(name.length, 5);
  name.copy(out, 7);
  return out;
}

export function encodeData(streamId: number, payload: Buffer): Buffer {
  const out = Buffer.alloc(1 + 4 + 4 + payload.length);
  out.writeUInt8(OP_DATA, 0);
  out.writeUInt32BE(streamId, 1);
  out.writeUInt32BE(payload.length, 5);
  payload.copy(out, 9);
  return out;
}

export function encodeClose(streamId: number): Buffer {
  const out = Buffer.alloc(1 + 4);
  out.writeUInt8(OP_CLOSE, 0);
  out.writeUInt32BE(streamId, 1);
  return out;
}

// --- decode ------------------------------------------------------------------------------------

export function parseFrame(buf: Buffer): ParsedFrame {
  if (buf.length < 1) throw new Error('empty frame');
  const op = buf[0];
  if (op === OP_OPEN) {
    if (buf.length < 7) throw new Error('bad OPEN frame');
    const streamId = buf.readUInt32BE(1);
    const nameLen = buf.readUInt16BE(5);
    if (buf.length < 7 + nameLen) throw new Error('truncated OPEN frame');
    const tunnelName = buf.subarray(7, 7 + nameLen).toString('utf8');
    return { op: 'open', streamId, tunnelName };
  }
  if (op === OP_DATA) {
    if (buf.length < 9) throw new Error('bad DATA frame');
    const streamId = buf.readUInt32BE(1);
    const len = buf.readUInt32BE(5);
    if (buf.length < 9 + len) throw new Error('truncated DATA frame');
    const payload = Buffer.from(buf.subarray(9, 9 + len));
    return { op: 'data', streamId, payload };
  }
  if (op === OP_CLOSE) {
    if (buf.length < 5) throw new Error('bad CLOSE frame');
    const streamId = buf.readUInt32BE(1);
    return { op: 'close', streamId };
  }
  throw new Error(`unknown frame op: ${op}`);
}

export function tryParseFrame(buf: Buffer): ParsedFrame | null {
  if (buf.length === 0) return null;
  try {
    return parseFrame(buf);
  } catch {
    return null;
  }
}
