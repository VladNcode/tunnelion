//! Wire format: PSK handshake then Yamux. Each logical tunnel stream starts with a small header
//! (version + UTF-8 tunnel name) before application bytes.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

type HmacSha256 = Hmac<Sha256>;

pub const MAGIC: &[u8; 8] = b"TUNNLN01";
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_TUNNEL_NAME_LEN: usize = 255;

pub const STREAM_HEADER_VERSION: u8 = 1;

const LABEL_CLIENT: &[u8] = b"tunnelion-handshake-client-v1\0";
const LABEL_SERVER: &[u8] = b"tunnelion-handshake-server-v1\0";

/// First message from server after TCP accept: magic + version + server random.
pub const SERVER_BANNER_LEN: usize = MAGIC.len() + 2 + 32;
/// Client response: magic + version + client random + HMAC-SHA256 output.
pub const CLIENT_PROOF_LEN: usize = MAGIC.len() + 2 + 32 + 32;
/// Final server message: HMAC-SHA256 output.
pub const SERVER_PROOF_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unexpected eof")]
    UnexpectedEof,
    #[error("bad magic")]
    BadMagic,
    #[error("unsupported protocol version {0}")]
    BadVersion(u16),
    #[error("invalid handshake")]
    InvalidHandshake,
    #[error("tunnel name too long (max {MAX_TUNNEL_NAME_LEN})")]
    TunnelNameTooLong,
    #[error("invalid stream header version {0}")]
    BadStreamVersion(u8),
    #[error("tunnel name is not valid UTF-8")]
    InvalidTunnelNameUtf8,
}

fn mac_client_proof(psk: &[u8], server_random: &[u8; 32], client_random: &[u8; 32]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(psk).expect("HMAC key length");
    mac.update(LABEL_CLIENT);
    mac.update(server_random);
    mac.update(client_random);
    let out = mac.finalize().into_bytes();
    out[..32].try_into().expect("hmac size")
}

fn mac_server_proof(psk: &[u8], server_random: &[u8; 32], client_random: &[u8; 32]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(psk).expect("HMAC key length");
    mac.update(LABEL_SERVER);
    mac.update(server_random);
    mac.update(client_random);
    let out = mac.finalize().into_bytes();
    out[..32].try_into().expect("hmac size")
}

/// Server side: send banner, read client proof, verify, send server proof.
pub async fn server_handshake<RW>(io: &mut RW, psk: &[u8]) -> Result<(), ProtocolError>
where
    RW: AsyncRead + AsyncWrite + Unpin,
{
    let mut server_random = [0u8; 32];
    getrandom_fill(&mut server_random);

    let mut banner = [0u8; SERVER_BANNER_LEN];
    banner[..MAGIC.len()].copy_from_slice(MAGIC.as_slice());
    banner[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    banner[MAGIC.len() + 2..].copy_from_slice(&server_random);
    io.write_all(&banner).await?;

    let mut client_msg = [0u8; CLIENT_PROOF_LEN];
    io.read_exact(&mut client_msg).await?;

    if client_msg[..MAGIC.len()] != MAGIC[..] {
        return Err(ProtocolError::BadMagic);
    }
    let ver = u16::from_be_bytes(client_msg[MAGIC.len()..MAGIC.len() + 2].try_into().unwrap());
    if ver != PROTOCOL_VERSION {
        return Err(ProtocolError::BadVersion(ver));
    }
    let client_random: [u8; 32] = client_msg[MAGIC.len() + 2..MAGIC.len() + 2 + 32]
        .try_into()
        .unwrap();
    let got_mac: [u8; 32] = client_msg[MAGIC.len() + 2 + 32..]
        .try_into()
        .unwrap();
    let expected = mac_client_proof(psk, &server_random, &client_random);
    if !bool::from(got_mac.ct_eq(&expected)) {
        return Err(ProtocolError::InvalidHandshake);
    }

    let server_mac = mac_server_proof(psk, &server_random, &client_random);
    io.write_all(&server_mac).await?;
    io.flush().await?;
    Ok(())
}

/// Client side: read banner, send proof, read server proof.
pub async fn client_handshake<RW>(io: &mut RW, psk: &[u8]) -> Result<(), ProtocolError>
where
    RW: AsyncRead + AsyncWrite + Unpin,
{
    let mut banner = [0u8; SERVER_BANNER_LEN];
    io.read_exact(&mut banner).await?;

    if banner[..MAGIC.len()] != MAGIC[..] {
        return Err(ProtocolError::BadMagic);
    }
    let ver = u16::from_be_bytes(banner[MAGIC.len()..MAGIC.len() + 2].try_into().unwrap());
    if ver != PROTOCOL_VERSION {
        return Err(ProtocolError::BadVersion(ver));
    }
    let server_random: [u8; 32] = banner[MAGIC.len() + 2..].try_into().unwrap();

    let mut client_random = [0u8; 32];
    getrandom_fill(&mut client_random);
    let mac = mac_client_proof(psk, &server_random, &client_random);

    let mut proof = [0u8; CLIENT_PROOF_LEN];
    proof[..MAGIC.len()].copy_from_slice(MAGIC.as_slice());
    proof[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    proof[MAGIC.len() + 2..MAGIC.len() + 2 + 32].copy_from_slice(&client_random);
    proof[MAGIC.len() + 2 + 32..].copy_from_slice(&mac);
    io.write_all(&proof).await?;
    io.flush().await?;

    let mut server_mac = [0u8; SERVER_PROOF_LEN];
    io.read_exact(&mut server_mac).await?;
    let expected = mac_server_proof(psk, &server_random, &client_random);
    if !bool::from(server_mac.ct_eq(&expected)) {
        return Err(ProtocolError::InvalidHandshake);
    }
    Ok(())
}

fn getrandom_fill(buf: &mut [u8]) {
    getrandom::getrandom(buf).expect("getrandom");
}

/// Encode stream preamble: version (1) + u16 len + name UTF-8.
pub fn encode_stream_header(tunnel_name: &str) -> Result<Vec<u8>, ProtocolError> {
    let b = tunnel_name.as_bytes();
    if b.len() > MAX_TUNNEL_NAME_LEN {
        return Err(ProtocolError::TunnelNameTooLong);
    }
    let len = b.len() as u16;
    let mut v = Vec::with_capacity(1 + 2 + b.len());
    v.push(STREAM_HEADER_VERSION);
    v.extend_from_slice(&len.to_be_bytes());
    v.extend_from_slice(b);
    Ok(v)
}

/// Read stream preamble from the start of a new Yamux stream (Tokio `AsyncRead`).
pub async fn read_stream_header<R>(read: &mut R) -> Result<String, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut ver = [0u8; 1];
    read.read_exact(&mut ver).await?;
    if ver[0] != STREAM_HEADER_VERSION {
        return Err(ProtocolError::BadStreamVersion(ver[0]));
    }
    let mut len_b = [0u8; 2];
    read.read_exact(&mut len_b).await?;
    let n = u16::from_be_bytes(len_b) as usize;
    if n > MAX_TUNNEL_NAME_LEN {
        return Err(ProtocolError::TunnelNameTooLong);
    }
    let mut name = vec![0u8; n];
    if n > 0 {
        read.read_exact(&mut name).await?;
    }
    String::from_utf8(name).map_err(|_| ProtocolError::InvalidTunnelNameUtf8)
}

/// Read stream preamble (`futures::io::AsyncRead`, e.g. a Yamux substream).
pub async fn read_stream_header_futures<R>(read: &mut R) -> Result<String, ProtocolError>
where
    R: futures::io::AsyncRead + Unpin,
{
    use futures::io::AsyncReadExt;
    let mut ver = [0u8; 1];
    read.read_exact(&mut ver).await?;
    if ver[0] != STREAM_HEADER_VERSION {
        return Err(ProtocolError::BadStreamVersion(ver[0]));
    }
    let mut len_b = [0u8; 2];
    read.read_exact(&mut len_b).await?;
    let n = u16::from_be_bytes(len_b) as usize;
    if n > MAX_TUNNEL_NAME_LEN {
        return Err(ProtocolError::TunnelNameTooLong);
    }
    let mut name = vec![0u8; n];
    if n > 0 {
        read.read_exact(&mut name).await?;
    }
    String::from_utf8(name).map_err(|_| ProtocolError::InvalidTunnelNameUtf8)
}

/// Write stream preamble.
pub async fn write_stream_header<W>(write: &mut W, tunnel_name: &str) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
{
    let buf = encode_stream_header(tunnel_name)?;
    write.write_all(&buf).await?;
    write.flush().await?;
    Ok(())
}

/// Constant-time compare for tests / reuse.
pub fn ct_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    bool::from(a.ct_eq(b))
}

#[cfg(test)]
mod handshake_tests {
    use super::*;
    use tokio::io::duplex;
    use tokio::join;

    #[tokio::test]
    async fn handshake_ok() {
        let psk = b"unit-test-psk";
        let (mut a, mut b) = duplex(4096);
        let s = tokio::spawn(async move { server_handshake(&mut a, psk).await });
        let c = tokio::spawn(async move { client_handshake(&mut b, psk).await });
        let (sr, cr) = join!(s, c);
        sr.unwrap().unwrap();
        cr.unwrap().unwrap();
    }

    #[tokio::test]
    async fn handshake_wrong_psk() {
        let (mut a, mut b) = duplex(4096);
        let s = tokio::spawn(async move { server_handshake(&mut a, b"one").await });
        let c = tokio::spawn(async move { client_handshake(&mut b, b"two").await });
        let (sr, cr) = join!(s, c);
        assert!(sr.unwrap().is_err() || cr.unwrap().is_err());
    }

    #[tokio::test]
    async fn handshake_bad_client_mac() {
        let psk = b"psk";
        let (mut a, mut b) = duplex(4096);
        let server = tokio::spawn(async move { server_handshake(&mut a, psk).await });
        let mut banner = [0u8; SERVER_BANNER_LEN];
        b.read_exact(&mut banner).await.unwrap();
        let client_random = [9u8; 32];
        let mut proof = [0u8; CLIENT_PROOF_LEN];
        proof[..MAGIC.len()].copy_from_slice(MAGIC.as_slice());
        proof[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        proof[MAGIC.len() + 2..MAGIC.len() + 2 + 32].copy_from_slice(&client_random);
        // wrong MAC (zeros)
        b.write_all(&proof).await.unwrap();
        b.flush().await.unwrap();
        let r = server.await.unwrap();
        assert!(matches!(r, Err(ProtocolError::InvalidHandshake)));
    }
}

#[cfg(test)]
mod stream_header_tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn roundtrip_header() {
        let (mut r, mut w) = duplex(256);
        let t = tokio::spawn(async move { read_stream_header(&mut r).await });
        write_stream_header(&mut w, "messengers").await.unwrap();
        assert_eq!(t.await.unwrap().unwrap(), "messengers");
    }

    #[tokio::test]
    async fn encode_rejects_long_name() {
        let s = "x".repeat(MAX_TUNNEL_NAME_LEN + 1);
        assert!(encode_stream_header(&s).is_err());
    }
}

#[cfg(test)]
mod yaml_config_tests {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Relay {
        addr: String,
    }

    #[derive(Debug, Deserialize)]
    struct Tunnel {
        #[allow(dead_code)]
        addr: String,
        #[allow(dead_code)]
        domain: String,
    }

    #[derive(Debug, Deserialize)]
    struct Root {
        relay: Relay,
        tunnels: std::collections::BTreeMap<String, Tunnel>,
    }

    #[test]
    fn valid_single_tunnel() {
        let yaml = r#"
relay:
  addr: "127.0.0.1:9000"
tunnels:
  app:
    addr: "8080"
    domain: "app.example.com"
"#;
        let r: Root = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(r.relay.addr, "127.0.0.1:9000");
        assert_eq!(r.tunnels.len(), 1);
    }

    #[test]
    fn invalid_missing_tunnels() {
        let yaml = r#"
relay:
  addr: "127.0.0.1:1"
"#;
        let e = serde_yaml::from_str::<Root>(yaml).unwrap_err();
        assert!(e.to_string().contains("missing") || e.to_string().contains("tunnels"));
    }

    #[test]
    fn valid_two_tunnels() {
        let yaml = r#"
relay:
  addr: "127.0.0.1:1"
tunnels:
  a:
    addr: "1"
    domain: "a"
  b:
    addr: "127.0.0.1:2"
    domain: "b"
"#;
        let r: Root = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(r.tunnels.len(), 2);
    }
}
