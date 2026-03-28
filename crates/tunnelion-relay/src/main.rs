use anyhow::{Context, Result};
use std::env;
use std::fs;
use tracing_subscriber::EnvFilter;
use tunnelion_relay::{RelayConfig, parse_relay_yaml, relay_file_with_psk, run_relay};

fn load_config() -> Result<RelayConfig> {
    let psk = env::var("TUNNELION_PSK")
        .context("TUNNELION_PSK is required")?
        .into_bytes();

    let path = env::var("TUNNELION_RELAY_CONFIG").unwrap_or_else(|_| "relay.yaml".into());
    let s = fs::read_to_string(&path).with_context(|| {
        format!("read relay config from {path} (create relay.yaml or set TUNNELION_RELAY_CONFIG)")
    })?;
    let file = parse_relay_yaml(&s).with_context(|| format!("parse {path}"))?;
    Ok(relay_file_with_psk(file, psk))
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
