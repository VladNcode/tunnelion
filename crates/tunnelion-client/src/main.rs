use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tunnelion_client::{ClientConfig, parse_client_yaml, run_client, run_client_with_tui};

fn main() -> Result<()> {
    let headless = env::var("TUNNELION_HEADLESS").ok().as_deref() == Some("1");

    // Tracing prints to stderr; any `info!` / `warn!` corrupts Ratatui's alternate screen.
    // Headless: default to useful client logs. TUI: stderr silent unless RUST_LOG is set.
    let filter = match env::var("RUST_LOG") {
        Ok(_) => EnvFilter::from_default_env(),
        Err(_) => {
            if headless {
                EnvFilter::new("tunnelion_client=info,warn")
            } else {
                EnvFilter::new("off")
            }
        }
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let path: PathBuf = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tunnels.yaml"));
    let yaml =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut cfg: ClientConfig = parse_client_yaml(&yaml)?;
    let psk = env::var("TUNNELION_PSK").context("TUNNELION_PSK is required")?;
    cfg.psk = psk.into_bytes();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if headless {
        rt.block_on(run_client(cfg))
    } else {
        rt.block_on(run_client_with_tui(cfg))
    }
}
