/**
 * Parse `host:port` or `[ipv6]:port` for `net.listen` / `net.connect`.
 */

import { IPV6_HOST_PORT } from './constants.js';

export function parseHostPort(s: string): { host: string; port: number } {
  const t = s.trim();
  const ipv6 = IPV6_HOST_PORT.exec(t);
  if (ipv6) {
    return { host: ipv6[1], port: Number.parseInt(ipv6[2], 10) };
  }
  const idx = t.lastIndexOf(':');
  if (idx <= 0) {
    throw new Error(`invalid socket address: ${JSON.stringify(s)}`);
  }
  const port = Number.parseInt(t.slice(idx + 1), 10);
  if (!Number.isFinite(port)) {
    throw new Error(`invalid socket address: ${JSON.stringify(s)}`);
  }
  return { host: t.slice(0, idx), port };
}
