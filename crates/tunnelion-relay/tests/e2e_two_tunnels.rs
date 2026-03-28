//! Two tunnels: traffic on port A vs B must reach different local echo servers.

use std::net::TcpListener as StdTcpListener;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tunnelion_client::{parse_client_yaml, run_client, ClientConfig};
use tunnelion_relay::{RelayConfig, TunnelListen, run_relay};

fn ephemeral_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn prefix_echo_server(port: u16, prefix: &[u8]) {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let (mut tcp, _) = listener.accept().await.unwrap();
    let mut buf = [0u8; 64];
    let n = tcp.read(&mut buf).await.unwrap();
    tcp.write_all(prefix).await.unwrap();
    tcp.write_all(&buf[..n]).await.unwrap();
}

#[tokio::test]
async fn two_public_ports_route_to_distinct_locals() {
    let control_port = ephemeral_port();
    let tunnel_a = ephemeral_port();
    let tunnel_b = ephemeral_port();
    let echo_a = ephemeral_port();
    let echo_b = ephemeral_port();
    let psk = "e2e-two-psk";

    let echo_ha = tokio::spawn(prefix_echo_server(echo_a, b"A:"));
    let echo_hb = tokio::spawn(prefix_echo_server(echo_b, b"B:"));

    let relay_cfg = RelayConfig {
        control_listen: ([127, 0, 0, 1], control_port).into(),
        psk: psk.as_bytes().to_vec(),
        tunnels: vec![
            TunnelListen {
                name: "alpha".into(),
                listen: ([127, 0, 0, 1], tunnel_a).into(),
            },
            TunnelListen {
                name: "beta".into(),
                listen: ([127, 0, 0, 1], tunnel_b).into(),
            },
        ],
    };
    let relay_h = tokio::spawn(run_relay(relay_cfg));

    let yaml = format!(
        r#"
relay:
  addr: "127.0.0.1:{control_port}"
tunnels:
  alpha:
    addr: "{echo_a}"
    domain: "a.local"
  beta:
    addr: "{echo_b}"
    domain: "b.local"
"#
    );
    let mut client_cfg: ClientConfig = parse_client_yaml(&yaml).unwrap();
    client_cfg.psk = psk.as_bytes().to_vec();
    let client_h = tokio::spawn(run_client(client_cfg));

    let mut ca = None;
    let mut cb = None;
    for _ in 0..100 {
        if ca.is_none()
            && let Ok(t) = tokio::net::TcpStream::connect(("127.0.0.1", tunnel_a)).await
        {
            ca = Some(t);
        }
        if cb.is_none()
            && let Ok(t) = tokio::net::TcpStream::connect(("127.0.0.1", tunnel_b)).await
        {
            cb = Some(t);
        }
        if ca.is_some() && cb.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let mut ca = ca.expect("connect tunnel alpha");
    let mut cb = cb.expect("connect tunnel beta");

    ca.write_all(b"hello-a").await.unwrap();
    cb.write_all(b"hello-b").await.unwrap();

    let mut out_a = [0u8; 32];
    let mut out_b = [0u8; 32];
    let (na, nb) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(5), ca.read(&mut out_a)),
        tokio::time::timeout(Duration::from_secs(5), cb.read(&mut out_b)),
    );
    let na = na.expect("timeout a").unwrap();
    let nb = nb.expect("timeout b").unwrap();
    assert_eq!(&out_a[..na], b"A:hello-a");
    assert_eq!(&out_b[..nb], b"B:hello-b");

    relay_h.abort();
    client_h.abort();
    echo_ha.abort();
    echo_hb.abort();
}
