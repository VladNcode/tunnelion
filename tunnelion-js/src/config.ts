/**
 * JSON configuration (no YAML).
 */

import { readFileSync } from 'node:fs';

import { isObject } from './helpers.js';

// --- relay -----------------------------------------------------------------------------------

export interface RelayControlConfig {
  host: string;
  port: number;
}

export interface RelayTunnelEntry {
  listen: string;
}

export interface RelayConfig {
  control: RelayControlConfig;
  tunnels: Record<string, RelayTunnelEntry>;
}

export function loadRelayConfig(path: string): RelayConfig {
  const raw = JSON.parse(readFileSync(path, 'utf8')) as unknown;
  if (!isObject(raw)) throw new Error('relay config: root must be an object');
  const control = raw.control;
  if (!isObject(control)) throw new Error('relay config: missing control object');
  const host = control.host;
  const port = control.port;
  if (typeof host !== 'string' || host.trim() === '') {
    throw new Error('relay config: control.host must be a non-empty string');
  }
  if (typeof port !== 'number' || !Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error('relay config: control.port must be an integer 1–65535');
  }
  const tunnels = raw.tunnels;
  if (!isObject(tunnels) || Object.keys(tunnels).length === 0) {
    throw new Error('relay config: tunnels must be a non-empty object');
  }
  const out: Record<string, RelayTunnelEntry> = {};
  const seenListen = new Set<string>();
  for (const name of Object.keys(tunnels)) {
    const t = tunnels[name];
    if (!isObject(t)) throw new Error(`relay config: tunnel ${JSON.stringify(name)} must be an object`);
    const listen = t.listen;
    if (typeof listen !== 'string' || listen.trim() === '') {
      throw new Error(`relay config: tunnel ${JSON.stringify(name)}.listen required`);
    }
    if (seenListen.has(listen)) {
      throw new Error(`relay config: duplicate listen ${JSON.stringify(listen)}`);
    }
    seenListen.add(listen);
    out[name] = { listen };
  }
  return { control: { host: host.trim(), port }, tunnels: out };
}

// --- client ----------------------------------------------------------------------------------

export interface ClientRelayConfig {
  url: string;
}

export interface ClientTunnelEntry {
  upstream: string;
}

export interface ClientConfig {
  relay: ClientRelayConfig;
  tunnels: Record<string, ClientTunnelEntry>;
}

export function loadClientConfig(path: string): ClientConfig {
  const raw = JSON.parse(readFileSync(path, 'utf8')) as unknown;
  if (!isObject(raw)) throw new Error('client config: root must be an object');
  const relay = raw.relay;
  if (!isObject(relay)) throw new Error('client config: missing relay object');
  const url = relay.url;
  if (typeof url !== 'string' || url.trim() === '') {
    throw new Error('client config: relay.url must be a non-empty string');
  }
  const tunnels = raw.tunnels;
  if (!isObject(tunnels) || Object.keys(tunnels).length === 0) {
    throw new Error('client config: tunnels must be a non-empty object');
  }
  const out: Record<string, ClientTunnelEntry> = {};
  for (const name of Object.keys(tunnels)) {
    const t = tunnels[name];
    if (!isObject(t)) throw new Error(`client config: tunnel ${JSON.stringify(name)} must be an object`);
    const upstream = t.upstream;
    if (typeof upstream !== 'string' || upstream.trim() === '') {
      throw new Error(`client config: tunnel ${JSON.stringify(name)}.upstream required`);
    }
    out[name] = { upstream: upstream.trim() };
  }
  return { relay: { url: url.trim() }, tunnels: out };
}
