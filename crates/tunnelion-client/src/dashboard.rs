//! Shared atomic stats for the TUI (safe to read from the draw loop every tick).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use anyhow::Result;

use crate::ClientConfig;

/// Control session lifecycle for the status line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ControlState {
    Starting = 0,
    Connecting = 1,
    Live = 2,
    Reconnecting = 3,
    Down = 4,
}

pub struct TunnelRowStats {
    pub name: String,
    /// Shown as in YAML (`addr` field).
    pub local_addr_display: String,
    pub domain: String,
    /// Bytes read from Yamux (from the internet / relay).
    pub bytes_in: AtomicU64,
    /// Bytes written to Yamux (toward the internet / relay).
    pub bytes_out: AtomicU64,
    pub active_streams: AtomicU32,
}

/// Decrements `active_streams` when dropped (one active tunnel stream ended).
pub struct ActiveStreamGuard {
    row: Arc<TunnelRowStats>,
}

impl Drop for ActiveStreamGuard {
    fn drop(&mut self) {
        self.row.active_streams.fetch_sub(1, Ordering::Relaxed);
    }
}

impl TunnelRowStats {
    pub fn begin_stream(self: &Arc<Self>) -> ActiveStreamGuard {
        self.active_streams.fetch_add(1, Ordering::Relaxed);
        ActiveStreamGuard {
            row: Arc::clone(self),
        }
    }
}

pub struct DashboardStats {
    pub relay_display: String,
    control: AtomicU8,
    pub tunnels: Vec<Arc<TunnelRowStats>>,
    /// name -> row for dispatch in the data plane
    pub by_name: std::collections::BTreeMap<String, Arc<TunnelRowStats>>,
}

impl DashboardStats {
    pub fn new(cfg: &ClientConfig) -> Result<Self> {
        let mut tunnels = Vec::new();
        let mut by_name = std::collections::BTreeMap::new();
        for (name, t) in &cfg.tunnels {
            let row = Arc::new(TunnelRowStats {
                name: name.clone(),
                local_addr_display: t.addr.clone(),
                domain: t.domain.clone(),
                bytes_in: AtomicU64::new(0),
                bytes_out: AtomicU64::new(0),
                active_streams: AtomicU32::new(0),
            });
            by_name.insert(name.clone(), Arc::clone(&row));
            tunnels.push(row);
        }
        Ok(Self {
            relay_display: cfg.relay_addr.to_string(),
            control: AtomicU8::new(ControlState::Starting as u8),
            tunnels,
            by_name,
        })
    }

    pub fn set_control(&self, s: ControlState) {
        self.control.store(s as u8, Ordering::Relaxed);
    }

    pub fn control_state(&self) -> ControlState {
        match self.control.load(Ordering::Relaxed) {
            1 => ControlState::Connecting,
            2 => ControlState::Live,
            3 => ControlState::Reconnecting,
            4 => ControlState::Down,
            _ => ControlState::Starting,
        }
    }
}

pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MiB", n as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_smoke() {
        assert!(format_bytes(0).contains('B'));
        assert!(format_bytes(2048).contains("KiB"));
    }
}
