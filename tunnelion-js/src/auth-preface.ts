/**
 * Control-plane auth: one binary message after WebSocket open.
 * Use wss:// in production; the secret is sent in cleartext inside the WebSocket.
 */

import { timingSafeEqual } from 'node:crypto';

import { AUTH_PREFACE_HEADER_LEN, TN3V1_MAGIC } from './constants.js';

// --- client → server ---------------------------------------------------------------------------

/** Client builds: MAGIC + u16 BE secret length + UTF-8 secret */
export function encodeAuthPreface(secretUtf8: string): Buffer {
  const secret = Buffer.from(secretUtf8, 'utf8');
  if (secret.length > 65535) {
    throw new Error('secret too long (max 65535 bytes UTF-8)');
  }
  const out = Buffer.alloc(AUTH_PREFACE_HEADER_LEN + secret.length);
  TN3V1_MAGIC.copy(out, 0);
  out.writeUInt16BE(secret.length, TN3V1_MAGIC.length);
  secret.copy(out, AUTH_PREFACE_HEADER_LEN);
  return out;
}

// --- server: parse -----------------------------------------------------------------------------

/**
 * Parse first client payload. Returns secret bytes or null if malformed.
 */
export function tryParseAuthPreface(payload: Buffer): Buffer | null {
  if (payload.length < AUTH_PREFACE_HEADER_LEN) {
    return null;
  }
  if (!payload.subarray(0, TN3V1_MAGIC.length).equals(TN3V1_MAGIC)) {
    return null;
  }
  const len = payload.readUInt16BE(TN3V1_MAGIC.length);
  if (payload.length !== AUTH_PREFACE_HEADER_LEN + len) {
    return null;
  }
  return Buffer.from(payload.subarray(AUTH_PREFACE_HEADER_LEN));
}

// --- crypto ------------------------------------------------------------------------------------

/** Constant-time compare when lengths match; false if lengths differ. */
export function secretsEqual(a: Buffer, b: Buffer): boolean {
  if (a.length !== b.length) {
    return false;
  }
  return timingSafeEqual(a, b);
}
