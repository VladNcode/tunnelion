//! Localhost relay + client + echo upstream (no Caddy).

use std::net::TcpListener as StdTcpListener;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tunnelion_client::{ClientConfig, parse_client_yaml, run_client};
use tunnelion_relay::{RelayConfig, TunnelListen, run_relay};

fn ephemeral_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn public_port_reaches_local_echo() {
    let control_port = ephemeral_port();
    let tunnel_port = ephemeral_port();
    let echo_port = ephemeral_port();
    let psk = "e2e-test-psk";

    let echo_srv = tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", echo_port)).await.unwrap();
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 64];
        let n = tcp.read(&mut buf).await.unwrap();
        tcp.write_all(&buf[..n]).await.unwrap();
    });

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

    let mut pub_tcp = None;
    for _ in 0..80 {
        match tokio::net::TcpStream::connect(("127.0.0.1", tunnel_port)).await {
            Ok(t) => {
                pub_tcp = Some(t);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    }
    let mut pub_tcp = pub_tcp.expect("connect to tunnel port (timed out)");

    pub_tcp.write_all(b"hello-tunnelion").await.unwrap();
    let mut buf = [0u8; 32];
    let n = tokio::time::timeout(Duration::from_secs(5), pub_tcp.read(&mut buf))
        .await
        .expect("read timeout")
        .unwrap();
    assert_eq!(&buf[..n], b"hello-tunnelion");

    relay_h.abort();
    client_h.abort();
    echo_srv.abort();
}
