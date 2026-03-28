//! High concurrency through one tunnel; bounded wall-clock timeout.

use std::net::TcpListener as StdTcpListener;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tunnelion_client::{ClientConfig, parse_client_yaml, run_client};
use tunnelion_relay::{RelayConfig, TunnelListen, run_relay};

/// Parallel public connections; keep modest so CI stays predictable.
const CONCURRENT: usize = 24;

fn ephemeral_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn echo_per_connection(port: u16) {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    loop {
        let (mut tcp, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            let Ok(n) = tcp.read(&mut buf).await else {
                return;
            };
            if n == 0 {
                return;
            }
            let _ = tcp.write_all(&buf[..n]).await;
        });
    }
}

#[tokio::test]
async fn concurrent_public_connections_echo() {
    let control_port = ephemeral_port();
    let tunnel_port = ephemeral_port();
    let echo_port = ephemeral_port();
    let psk = "e2e-concurrent-psk";

    let echo_srv = tokio::spawn(echo_per_connection(echo_port));

    let relay_cfg = RelayConfig {
        control_listen: ([127, 0, 0, 1], control_port).into(),
        psk: psk.as_bytes().to_vec(),
        tunnels: vec![TunnelListen {
            name: "app".into(),
            listen: ([127, 0, 0, 1], tunnel_port).into(),
        }],
    };
    let relay_h = tokio::spawn(run_relay(relay_cfg));

    let yaml = format!(
        r#"
relay:
  addr: "127.0.0.1:{control_port}"
tunnels:
  app:
    addr: "{echo_port}"
    domain: "test.local"
"#
    );
    let mut client_cfg: ClientConfig = parse_client_yaml(&yaml).unwrap();
    client_cfg.psk = psk.as_bytes().to_vec();
    let client_h = tokio::spawn(run_client(client_cfg));

    let mut ready = false;
    for _ in 0..120 {
        if tokio::net::TcpStream::connect(("127.0.0.1", tunnel_port))
            .await
            .is_ok()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(ready, "tunnel port did not become connectable in time");

    let work = async {
        let mut handles = Vec::with_capacity(CONCURRENT);
        for i in 0..CONCURRENT {
            let port = tunnel_port;
            handles.push(tokio::spawn(async move {
                let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .unwrap();
                let payload = format!("c{i:04}");
                s.write_all(payload.as_bytes()).await.unwrap();
                let mut out = [0u8; 16];
                let n = tokio::time::timeout(Duration::from_secs(10), s.read(&mut out))
                    .await
                    .expect("per-connection read timeout (10s)")
                    .unwrap();
                assert_eq!(&out[..n], payload.as_bytes(), "echo mismatch for {i}");
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    };

    tokio::time::timeout(Duration::from_secs(45), work)
        .await
        .expect("overall test deadline (45s)");

    relay_h.abort();
    client_h.abort();
    echo_srv.abort();
}
