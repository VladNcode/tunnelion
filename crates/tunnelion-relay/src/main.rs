use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use tracing_subscriber::EnvFilter;
use tunnelion_relay::{
    parse_relay_yaml, relay_file_with_psk, RelayConfig, TunnelListen, run_relay,
};

fn load_config() -> Result<RelayConfig> {
    let psk = env::var("TUNNELION_PSK")
        .context("TUNNELION_PSK is required")?
        .into_bytes();

    let path = env::var("TUNNELION_RELAY_CONFIG").unwrap_or_else(|_| "relay.yaml".into());

    if Path::new(&path).try_exists().context("relay config path")? {
        let s = fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
        let file = parse_relay_yaml(&s).with_context(|| format!("parse {path}"))?;
        return Ok(relay_file_with_psk(file, psk));
    }

    // Back-compat: single tunnel from env when relay.yaml is missing
    let control_listen: SocketAddr = env::var("TUNNELION_CONTROL_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:9780".into())
        .parse()
        .context("TUNNELION_CONTROL_ADDR")?;
    let tunnel_name = env::var("TUNNELION_TUNNEL_NAME").context(
        "TUNNELION_TUNNEL_NAME is required when relay.yaml is missing (or create relay.yaml)",
    )?;
    let tunnel_listen: SocketAddr = env::var("TUNNELION_TUNNEL_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:18001".into())
        .parse()
        .context("TUNNELION_TUNNEL_LISTEN")?;

    Ok(RelayConfig {
        control_listen,
        psk,
        tunnels: vec![TunnelListen {
            name: tunnel_name,
            listen: tunnel_listen,
        }],
    })
}

fn main() -> Result<()> {
    let filter = match std::env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_default_env(),
        Err(_) => EnvFilter::new("tunnelion_relay=info,warn"),
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cfg = load_config()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_relay(cfg))
}
