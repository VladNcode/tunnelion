//! VPS relay: PSK handshake, Yamux server, tunnel TCP listeners → outbound Yamux streams.

use anyhow::{Context, Result, anyhow};
use futures::future::poll_fn;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::{FutureExt, select};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tracing::{debug, error, info, warn};
use tunnelion_protocol::{encode_stream_header, server_handshake};
use yamux::{Config, Connection, Mode};

/// One tunnel: public listen address and logical name (must match client YAML keys).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TunnelListen {
    pub name: String,
    pub listen: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub control_listen: SocketAddr,
    pub psk: Vec<u8>,
    pub tunnels: Vec<TunnelListen>,
}

/// Parsed `relay.yaml` body (no PSK; supply separately from the environment).
#[derive(Debug)]
pub struct RelayFileConfig {
    pub control_listen: SocketAddr,
    pub tunnels: Vec<TunnelListen>,
}

#[derive(Debug, Deserialize)]
struct RelayYamlRoot {
    control: ControlYaml,
    tunnels: BTreeMap<String, TunnelListenYaml>,
}

#[derive(Debug, Deserialize)]
struct ControlYaml {
    addr: String,
}

#[derive(Debug, Deserialize)]
struct TunnelListenYaml {
    listen: String,
}

/// Parse `relay.yaml` content and validate tunnel names and listen addresses.
pub fn parse_relay_yaml(s: &str) -> Result<RelayFileConfig> {
    let root: RelayYamlRoot = serde_yaml::from_str(s).context("parse relay.yaml")?;
    if root.tunnels.is_empty() {
        anyhow::bail!("relay: tunnels: must contain at least one tunnel");
    }
    let control_listen: SocketAddr = root
        .control
        .addr
        .parse()
        .context("control.addr must be a host:port socket address")?;
    let mut tunnels = Vec::new();
    for (name, entry) in root.tunnels {
        let listen: SocketAddr = entry
            .listen
            .parse()
            .with_context(|| format!("tunnels.{name}.listen must be a socket address"))?;
        tunnels.push(TunnelListen { name, listen });
    }
    validate_tunnel_list(&tunnels)?;
    Ok(RelayFileConfig {
        control_listen,
        tunnels,
    })
}

/// Turn file config + PSK into a [`RelayConfig`].
pub fn relay_file_with_psk(file: RelayFileConfig, psk: Vec<u8>) -> RelayConfig {
    RelayConfig {
        control_listen: file.control_listen,
        psk,
        tunnels: file.tunnels,
    }
}

/// Fail on empty list, duplicate tunnel names, or duplicate listen addresses.
pub fn validate_tunnel_list(tunnels: &[TunnelListen]) -> Result<()> {
    if tunnels.is_empty() {
        anyhow::bail!("relay: need at least one tunnel");
    }
    let mut seen_names = HashSet::new();
    let mut seen_addrs = HashSet::new();
    for t in tunnels {
        if !seen_names.insert(&t.name) {
            anyhow::bail!("relay: duplicate tunnel name {:?}", t.name);
        }
        if !seen_addrs.insert(t.listen) {
            anyhow::bail!(
                "relay: duplicate listen address {} (each tunnel needs a distinct port)",
                t.listen
            );
        }
    }
    Ok(())
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

pub async fn run_relay(cfg: RelayConfig) -> Result<()> {
    validate_tunnel_list(&cfg.tunnels)?;

    let control = TcpListener::bind(cfg.control_listen)
        .await
        .with_context(|| format!("bind control {}", cfg.control_listen))?;
    info!(addr = %cfg.control_listen, "control listening");

    let mut tunnel_listeners: Vec<(TunnelListen, Arc<TcpListener>)> = Vec::new();
    for t in &cfg.tunnels {
        let listener = TcpListener::bind(t.listen)
            .await
            .with_context(|| format!("bind tunnel {} on {}", t.name, t.listen))?;
        info!(addr = %t.listen, name = %t.name, "tunnel listening");
        tunnel_listeners.push((t.clone(), Arc::new(listener)));
    }

    loop {
        let (sock, peer) = control
            .accept()
            .await
            .context("accept control connection")?;
        info!(%peer, "control peer connected");
        sock.set_nodelay(true).ok();

        let mut io = sock;
        if let Err(e) = server_handshake(&mut io, &cfg.psk).await {
            warn!(
                %peer,
                error = %e,
                "control handshake failed; closing TCP (wrong TUNNELION_PSK, truncated client, or not a tunnelion client)"
            );
            continue;
        }
        info!(%peer, "control handshake ok, Yamux session starting");

        if let Err(e) = run_one_yamux_session(io, &tunnel_listeners).await {
            warn!(%peer, error = %e, "control / Yamux session ended with error");
        } else {
            info!(%peer, "control session finished (client disconnected or Yamux closed)");
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn run_one_yamux_session(
    io: TcpStream,
    tunnel_listeners: &[(TunnelListen, Arc<TcpListener>)],
) -> Result<()> {
    let compat = io.compat();
    let conn = Connection::new(compat, Config::default(), Mode::Server);
    let conn = Arc::new(std::sync::Mutex::new(conn));
    let stopped = Arc::new(AtomicBool::new(false));

    let (shutdown_tx, _) = broadcast::channel(16);

    let driver = {
        let conn = Arc::clone(&conn);
        let stopped = Arc::clone(&stopped);
        let shutdown_tx = shutdown_tx.clone();
        tokio::task::spawn(async move {
            drive_yamux_inbound(conn, stopped, shutdown_tx).await;
        })
    };

    let mut accept_handles = Vec::new();
    for (t, listener) in tunnel_listeners {
        let listener = Arc::clone(listener);
        let conn = Arc::clone(&conn);
        let shutdown_tx = shutdown_tx.clone();
        let shutdown_rx = shutdown_tx.subscribe();
        let tunnel_name = t.name.clone();
        let shutdown_tx_loop = shutdown_tx.clone();
        accept_handles.push(tokio::spawn(async move {
            tunnel_accept_loop(listener, conn, tunnel_name, shutdown_tx_loop, shutdown_rx).await
        }));
    }

    let (driver_res, accepts_res) = tokio::join!(driver, async move {
        let outs = futures::future::join_all(accept_handles).await;
        for o in outs {
            o.context("tunnel accept task join")??;
        }
        Ok::<(), anyhow::Error>(())
    });
    driver_res.context("yamux driver task")?;
    accepts_res?;
    stopped.store(true, Ordering::SeqCst);
    let _ = shutdown_tx.send(());
    Ok(())
}

async fn tunnel_accept_loop(
    listener: Arc<TcpListener>,
    conn: Arc<std::sync::Mutex<YamuxConn>>,
    tunnel_name: String,
    shutdown_tx: broadcast::Sender<()>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    loop {
        tokio::select! {
            biased;
            r = shutdown_rx.recv() => {
                match r {
                    Ok(()) | Err(broadcast::error::RecvError::Closed) => {
                        info!(tunnel = %tunnel_name, "relay: tunnel listener stopped");
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            accept = listener.accept() => {
                let (public_tcp, peer) =
                    accept.with_context(|| format!("relay: tunnel {} accept()", tunnel_name))?;
                public_tcp.set_nodelay(true).ok();
                let session = next_session_id();
                info!(
                    session,
                    %peer,
                    tunnel = %tunnel_name,
                    "relay: new public TCP connection"
                );

                let mut stream = match open_outbound(&conn).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(tunnel = %tunnel_name, "open outbound yamux stream: {e}");
                        let _ = shutdown_tx.send(());
                        return Err(e);
                    }
                };

                if let Err(e) = write_stream_header_on_futures(&mut stream, &tunnel_name).await {
                    error!("write stream header: {e}");
                    continue;
                }

                let conn_for_copy = Arc::clone(&conn);
                let tn = tunnel_name.clone();
                tokio::task::spawn(async move {
                    let started = Instant::now();
                    info!(session, %peer, tunnel = %tn, "relay: tunnel stream open, piping bytes");
                    match bridge_public_to_stream(public_tcp, stream, conn_for_copy, session).await {
                        Ok(()) => info!(
                            session,
                            tunnel = %tn,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "relay: tunnel session finished"
                        ),
                        Err(e) => warn!(
                            session,
                            tunnel = %tn,
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "relay: tunnel session error: {e}"
                        ),
                    }
                });
            }
        }
    }
    Ok(())
}

async fn drive_yamux_inbound(
    conn: Arc<std::sync::Mutex<YamuxConn>>,
    stopped: Arc<AtomicBool>,
    shutdown_tx: broadcast::Sender<()>,
) {
    loop {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        let inbound = poll_fn(|cx| {
            conn.lock()
                .expect("yamux mutex poisoned")
                .poll_next_inbound(cx)
        })
        .await;
        match inbound {
            None => {
                info!("yamux control closed (inbound none)");
                stopped.store(true, Ordering::SeqCst);
                let _ = shutdown_tx.send(());
                break;
            }
            Some(Err(e)) => {
                warn!("yamux inbound error: {e}");
                stopped.store(true, Ordering::SeqCst);
                let _ = shutdown_tx.send(());
                break;
            }
            Some(Ok(stream)) => {
                warn!("unexpected inbound yamux stream from client; closing");
                drop(stream);
            }
        }
    }
}

async fn open_outbound(conn: &Arc<std::sync::Mutex<YamuxConn>>) -> Result<yamux::Stream> {
    poll_fn(|cx| {
        conn.lock()
            .expect("yamux mutex poisoned")
            .poll_new_outbound(cx)
    })
    .await
    .map_err(|e| anyhow!("{e}"))
}

async fn write_stream_header_on_futures(
    stream: &mut yamux::Stream,
    tunnel_name: &str,
) -> Result<()> {
    let buf = encode_stream_header(tunnel_name)?;
    stream.write_all(&buf).await.map_err(|e| anyhow!(e))?;
    stream.flush().await.map_err(|e| anyhow!(e))?;
    Ok(())
}

async fn bridge_public_to_stream(
    public: TcpStream,
    mut y_stream: yamux::Stream,
    _conn: Arc<std::sync::Mutex<YamuxConn>>,
    session: u64,
) -> Result<()> {
    let (pub_r, pub_w) = public.into_split();
    let mut pub_r = pub_r.compat();
    let mut pub_w = pub_w.compat_write();
    let mut buf_p = vec![0u8; 16 * 1024];
    let mut buf_y = vec![0u8; 16 * 1024];
    let mut tcp_in_closed = false;
    let mut logged_first_public = false;

    loop {
        if tcp_in_closed {
            let n = y_stream.read(&mut buf_y).await.map_err(|e| anyhow!(e))?;
            if n == 0 {
                break;
            }
            pub_w.write_all(&buf_y[..n]).await.map_err(|e| anyhow!(e))?;
            continue;
        }

        select! {
            r = pub_r.read(&mut buf_p).fuse() => {
                let n = r.map_err(|e| anyhow!(e))?;
                if n == 0 {
                    tcp_in_closed = true;
                    let _ = y_stream.close().await;
                    continue;
                }
                if !logged_first_public {
                    logged_first_public = true;
                    debug!(
                        session,
                        preview = %lossy_preview(&buf_p[..n], 200),
                        "relay: first bytes from public"
                    );
                }
                y_stream.write_all(&buf_p[..n]).await.map_err(|e| anyhow!(e))?;
            }
            r = y_stream.read(&mut buf_y).fuse() => {
                let n = r.map_err(|e| anyhow!(e))?;
                if n == 0 {
                    break;
                }
                pub_w.write_all(&buf_y[..n]).await.map_err(|e| anyhow!(e))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_duplicate_listen() {
        let tunnels = vec![
            TunnelListen {
                name: "a".into(),
                listen: "127.0.0.1:1".parse().unwrap(),
            },
            TunnelListen {
                name: "b".into(),
                listen: "127.0.0.1:1".parse().unwrap(),
            },
        ];
        let e = validate_tunnel_list(&tunnels).unwrap_err();
        assert!(e.to_string().contains("duplicate listen"));
    }

    #[test]
    fn validate_rejects_duplicate_name() {
        let tunnels = vec![
            TunnelListen {
                name: "x".into(),
                listen: "127.0.0.1:1".parse().unwrap(),
            },
            TunnelListen {
                name: "x".into(),
                listen: "127.0.0.1:2".parse().unwrap(),
            },
        ];
        let e = validate_tunnel_list(&tunnels).unwrap_err();
        assert!(e.to_string().contains("duplicate tunnel name"));
    }

    #[test]
    fn parse_relay_yaml_roundtrip() {
        let yaml = r#"
control:
  addr: "127.0.0.1:9000"
tunnels:
  app:
    listen: "127.0.0.1:8001"
  api:
    listen: "127.0.0.1:8002"
"#;
        let f = parse_relay_yaml(yaml).unwrap();
        assert_eq!(f.control_listen.port(), 9000);
        assert_eq!(f.tunnels.len(), 2);
    }
}
