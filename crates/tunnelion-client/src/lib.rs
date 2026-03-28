//! Mac (or dev) client: YAML config, PSK handshake, Yamux client, tunnel streams → local TCP.
//! If the control connection drops, [`run_client`] retries with bounded exponential backoff; see the repo `README.md`.

mod dashboard;
mod tui;

use anyhow::{anyhow, Context, Result};
use futures::future::poll_fn;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::{select, FutureExt};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{debug, error, info, warn};
use tunnelion_protocol::{client_handshake, read_stream_header_futures, ProtocolError};
use yamux::{Config, Connection, Mode};

pub use dashboard::{DashboardStats, TunnelRowStats};

#[derive(Debug, Deserialize)]
struct RelayYaml {
    addr: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TunnelYaml {
    pub addr: String,
    pub domain: String,
}

#[derive(Debug, Deserialize)]
struct RootYaml {
    relay: RelayYaml,
    tunnels: BTreeMap<String, TunnelYaml>,
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub relay_addr: SocketAddr,
    pub psk: Vec<u8>,
    pub tunnels: BTreeMap<String, TunnelYaml>,
}

pub fn parse_client_yaml(s: &str) -> Result<ClientConfig> {
    let root: RootYaml = serde_yaml::from_str(s).context("parse tunnels.yaml")?;
    if root.tunnels.is_empty() {
        anyhow::bail!("tunnels: must contain at least one tunnel");
    }
    let relay_addr: SocketAddr = root
        .relay
        .addr
        .parse()
        .context("relay.addr must be a host:port socket address")?;
    Ok(ClientConfig {
        relay_addr,
        psk: Vec::new(),
        tunnels: root.tunnels,
    })
}

pub fn parse_local_socket(addr: &str) -> Result<SocketAddr> {
    let addr = addr.trim();
    if addr.contains(':') {
        addr.parse().context("invalid addr (expected host:port or port)")
    } else {
        let port: u16 = addr.parse().context("invalid port")?;
        Ok(SocketAddr::from(([127, 0, 0, 1], port)))
    }
}

type CompatConn = tokio_util::compat::Compat<TcpStream>;
type YamuxConn = Connection<CompatConn>;

fn next_session_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn lossy_preview(buf: &[u8], max: usize) -> String {
    let end = buf.len().min(max);
    String::from_utf8_lossy(&buf[..end])
        .chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
}

const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Headless client (tests, automation). No TUI. Reconnects forever with bounded exponential backoff.
pub async fn run_client(cfg: ClientConfig) -> Result<()> {
    run_client_reconnect_loop(cfg, None).await
}

/// Interactive full-screen dashboard + same tunneling as [`run_client`].
pub async fn run_client_with_tui(cfg: ClientConfig) -> Result<()> {
    let dash = Arc::new(DashboardStats::new(&cfg)?);
    let dash_net = Arc::clone(&dash);
    let net = tokio::spawn(async move {
        if let Err(e) = run_client_reconnect_loop(cfg, Some(dash_net)).await {
            tracing::error!("client: {e:#}");
        }
    });

    let res = tui::run_dashboard(dash).await;
    net.abort();
    let _ = net.await;
    res
}

fn build_name_to_local(cfg: &ClientConfig) -> Result<BTreeMap<String, SocketAddr>> {
    let mut name_to_local: BTreeMap<String, SocketAddr> = BTreeMap::new();
    for (name, tunnel) in &cfg.tunnels {
        let local = parse_local_socket(&tunnel.addr)
            .with_context(|| format!("tunnel {name:?} addr {:?}", tunnel.addr))?;
        name_to_local.insert(name.clone(), local);
    }
    Ok(name_to_local)
}

fn stderr_hint_for_error(e: &anyhow::Error) -> Option<String> {
    for cause in e.chain() {
        if let Some(p) = cause.downcast_ref::<ProtocolError>() {
            return Some(match p {
                ProtocolError::InvalidHandshake => {
                    "PSK handshake failed: secret does not match the relay (check TUNNELION_PSK on client and server)."
                        .to_string()
                }
                ProtocolError::BadMagic | ProtocolError::BadVersion(_) => {
                    "Handshake failed: connected host does not speak tunnelion on this port (wrong service or corrupt first bytes)."
                        .to_string()
                }
                ProtocolError::Io(io) => {
                    format!("Network I/O error during handshake: {io}")
                }
                ProtocolError::UnexpectedEof => {
                    "Handshake failed: connection closed before handshake finished (relay reset or TLS on the control port?)."
                        .to_string()
                }
                _ => format!("Protocol error: {p}"),
            });
        }
    }
    let root = e.to_string();
    if root.contains("connect relay") || root.contains("Connection refused") {
        Some(format!(
            "Cannot open TCP to relay at the configured address: {root}"
        ))
    } else {
        None
    }
}

/// One control TCP + Yamux session until the driver exits (disconnect). Handshake/connect failures are `Err`.
async fn run_single_control_session(
    cfg: &ClientConfig,
    dash: Option<&Arc<DashboardStats>>,
    name_to_local: &BTreeMap<String, SocketAddr>,
) -> Result<()> {
    let mut sock = TcpStream::connect(cfg.relay_addr)
        .await
        .with_context(|| format!("connect relay {}", cfg.relay_addr))?;
    sock.set_nodelay(true).ok();
    info!(addr = %cfg.relay_addr, "connected to relay");

    client_handshake(&mut sock, &cfg.psk)
        .await
        .map_err(anyhow::Error::from)?;

    if let Some(d) = dash {
        d.set_control(dashboard::ControlState::Live);
    }

    let compat = sock.compat();
    let conn = Connection::new(compat, Config::default(), Mode::Client);
    let conn = Arc::new(std::sync::Mutex::new(conn));
    let stopped = Arc::new(AtomicBool::new(false));

    let name_to_stats: Option<BTreeMap<String, Arc<TunnelRowStats>>> =
        dash.map(|d| d.by_name.clone());

    let driver = {
        let conn = Arc::clone(&conn);
        let stopped = Arc::clone(&stopped);
        let name_to_local = name_to_local.clone();
        tokio::task::spawn(async move {
            drive_inbound_streams(conn, stopped, name_to_local, name_to_stats).await;
        })
    };

    let _ = driver.await;
    stopped.store(true, Ordering::SeqCst);
    if let Some(d) = dash {
        d.set_control(dashboard::ControlState::Down);
    }
    Ok(())
}

async fn run_client_reconnect_loop(
    cfg: ClientConfig,
    dash: Option<Arc<DashboardStats>>,
) -> Result<()> {
    if cfg.psk.is_empty() {
        anyhow::bail!("PSK is empty (set from environment)");
    }

    let name_to_local = build_name_to_local(&cfg)?;
    let mut backoff = INITIAL_BACKOFF;
    let mut ever_connected = false;

    loop {
        if let Some(d) = &dash {
            d.set_control(if ever_connected {
                dashboard::ControlState::Reconnecting
            } else {
                dashboard::ControlState::Connecting
            });
        }

        match run_single_control_session(&cfg, dash.as_ref(), &name_to_local).await {
            Ok(()) => {
                ever_connected = true;
                warn!(
                    retry_in = ?INITIAL_BACKOFF,
                    "control session ended; reconnecting"
                );
                tokio::time::sleep(INITIAL_BACKOFF).await;
                backoff = INITIAL_BACKOFF;
            }
            Err(e) => {
                if dash.is_none() {
                    if let Some(hint) = stderr_hint_for_error(&e) {
                        eprintln!("tunnelion-client: {hint}");
                    }
                }
                warn!(error = %e, retry_in = ?backoff, "session setup failed; retrying");
                tokio::time::sleep(backoff).await;
                let next_ms = backoff.as_millis().saturating_mul(2);
                backoff = Duration::from_millis(
                    (next_ms as u64).min(MAX_BACKOFF.as_millis() as u64),
                );
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn drive_inbound_streams(
    conn: Arc<std::sync::Mutex<YamuxConn>>,
    stopped: Arc<AtomicBool>,
    name_to_local: BTreeMap<String, SocketAddr>,
    name_to_stats: Option<BTreeMap<String, Arc<TunnelRowStats>>>,
) {
    loop {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        let inbound = poll_fn(|cx| conn.lock().expect("yamux mutex poisoned").poll_next_inbound(cx)).await;
        match inbound {
            None => {
                info!("yamux session closed");
                stopped.store(true, Ordering::SeqCst);
                break;
            }
            Some(Err(e)) => {
                error!("yamux inbound: {e}");
                stopped.store(true, Ordering::SeqCst);
                break;
            }
            Some(Ok(mut stream)) => {
                let name = match read_stream_header_futures(&mut stream).await {
                    Ok(n) => n,
                    Err(e) => {
                        warn!("stream header: {e}");
                        continue;
                    }
                };
                let local_addr = match name_to_local.get(&name) {
                    Some(a) => *a,
                    None => {
                        warn!(
                            got = %name,
                            known = ?name_to_local.keys().collect::<Vec<_>>(),
                            "client: unknown tunnel name in stream header"
                        );
                        continue;
                    }
                };
                let row_stats = name_to_stats
                    .as_ref()
                    .and_then(|m| m.get(&name).cloned());
                let session = next_session_id();
                info!(
                    session,
                    tunnel = %name,
                    %local_addr,
                    "client: inbound tunnel stream (connecting upstream)"
                );
                tokio::task::spawn(async move {
                    let local_tcp = match TcpStream::connect(local_addr).await {
                        Ok(t) => t,
                        Err(e) => {
                            warn!(session, "client: upstream connect failed: {local_addr}: {e}");
                            return;
                        }
                    };
                    let started = Instant::now();
                    info!(session, %local_addr, "client: upstream connected, piping");
                    match bridge_stream_to_local(stream, local_tcp, session, row_stats).await {
                        Ok(()) => info!(
                            session,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "client: tunnel session finished"
                        ),
                        Err(e) => warn!(
                            session,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "client: tunnel session error: {e}"
                        ),
                    }
                });
            }
        }
    }
}

async fn bridge_stream_to_local(
    mut y_stream: yamux::Stream,
    local: TcpStream,
    session: u64,
    stats: Option<Arc<TunnelRowStats>>,
) -> Result<()> {
    let _stream_guard = stats.as_ref().map(|s| s.begin_stream());

    let (loc_r, loc_w) = local.into_split();
    let mut loc_r = loc_r.compat();
    let mut loc_w = loc_w.compat_write();
    let mut buf_l = vec![0u8; 16 * 1024];
    let mut buf_y = vec![0u8; 16 * 1024];
    let mut local_in_closed = false;
    let mut logged_first_from_tunnel = false;

    loop {
        if local_in_closed {
            let n = y_stream.read(&mut buf_y).await.map_err(|e| anyhow!(e))?;
            if n == 0 {
                break;
            }
            if let Some(s) = &stats {
                s.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
            }
            loc_w.write_all(&buf_y[..n]).await.map_err(|e| anyhow!(e))?;
            continue;
        }

        select! {
            r = loc_r.read(&mut buf_l).fuse() => {
                let n = r.map_err(|e| anyhow!(e))?;
                if n == 0 {
                    local_in_closed = true;
                    let _ = y_stream.close().await;
                    continue;
                }
                if let Some(s) = &stats {
                    s.bytes_out.fetch_add(n as u64, Ordering::Relaxed);
                }
                y_stream.write_all(&buf_l[..n]).await.map_err(|e| anyhow!(e))?;
            }
            r = y_stream.read(&mut buf_y).fuse() => {
                let n = r.map_err(|e| anyhow!(e))?;
                if n == 0 {
                    break;
                }
                if let Some(s) = &stats {
                    s.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
                }
                if !logged_first_from_tunnel {
                    logged_first_from_tunnel = true;
                    debug!(
                        session,
                        preview = %lossy_preview(&buf_y[..n], 200),
                        "client: first bytes from tunnel toward upstream"
                    );
                }
                loc_w.write_all(&buf_y[..n]).await.map_err(|e| anyhow!(e))?;
            }
        }
    }
    Ok(())
}
