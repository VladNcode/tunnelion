/**
 * Wire markers, frame opcodes, and runtime tuning (payload limits, reconnect backoff).
 */

// --- WebSocket --------------------------------------------------------------------------------

export const WS_MAX_PAYLOAD = 100 * 1024 * 1024;

// --- Client reconnect -------------------------------------------------------------------------

export const SESSION_END_MS = 500;
export const FAIL_INITIAL_MS = 500;
export const FAIL_MAX_MS = 30_000;

// --- Frame opcodes (binary, after auth) -------------------------------------------------------

export const OP_OPEN = 0x01;
export const OP_DATA = 0x02;
export const OP_CLOSE = 0x03;

// --- Auth preface (TN3V1) ---------------------------------------------------------------------

export const TN3V1_MAGIC = Buffer.from('TN3V1', 'ascii');
export const AUTH_PREFACE_HEADER_LEN = TN3V1_MAGIC.length + 2;

/** Server → client after a valid preface. */
export const SERVER_AUTH_OK = Buffer.from('OK', 'ascii');

// --- Socket addresses (`host:port` / `[v6]:port`) ---------------------------------------------

export const IPV6_HOST_PORT = /^\[(.+)]:(\d+)$/;
