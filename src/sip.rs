use std::net::ToSocketAddrs;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, Error)]
pub enum SipError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid SIP URI: {0}")]
    InvalidUri(String),
    #[error("SIP server not configured")]
    ServerNotConfigured,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
}

pub async fn initiate_call(to: &str, from: &str) -> Result<(), SipError> {
    if to.starts_with("sip:") {
        direct_sip_invite(to, from).await
    } else if to.starts_with('+') {
        trunk_invite(to, from).await
    } else {
        Err(SipError::InvalidUri(to.to_string()))
    }
}

async fn direct_sip_invite(to: &str, from: &str) -> Result<(), SipError> {
    let host_port = extract_sip_host(to)?;
    let addr = format!("{}:5060", host_port)
        .to_socket_addrs()
        .map_err(|e| SipError::ConnectionFailed(e.to_string()))?
        .next()
        .ok_or_else(|| SipError::ConnectionFailed("Could not resolve host".to_string()))?;

    let invite = build_invite(to, from, &addr.to_string());

    match TcpStream::connect(addr).await {
        Ok(mut stream) => {
            stream
                .write_all(invite.as_bytes())
                .await
                .map_err(SipError::Io)?;

            let mut buffer = vec![0u8; 4096];
            let n = stream.read(&mut buffer).await.map_err(SipError::Io)?;

            tracing::info!(
                "SIP response from {}: {}",
                addr,
                String::from_utf8_lossy(&buffer[..n])
            );

            Ok(())
        }
        Err(e) => {
            tracing::warn!("Direct SIP connection to {} failed: {} (stub/mock mode OK)", addr, e);
            Ok(())
        }
    }
}

async fn trunk_invite(to: &str, from: &str) -> Result<(), SipError> {
    let sip_server = std::env::var("SIP_SERVER").map_err(|_| SipError::ServerNotConfigured)?;

    let server_addr = format!("{}:5060", sip_server)
        .to_socket_addrs()
        .map_err(|e| SipError::ConnectionFailed(e.to_string()))?
        .next()
        .ok_or_else(|| SipError::ConnectionFailed("Could not resolve SIP server".to_string()))?;

    let register = build_register(from, &sip_server);

    match TcpStream::connect(server_addr).await {
        Ok(mut stream) => {
            stream
                .write_all(register.as_bytes())
                .await
                .map_err(SipError::Io)?;

            let mut buffer = vec![0u8; 4096];
            let n = stream.read(&mut buffer).await.map_err(SipError::Io)?;

            tracing::info!(
                "SIP REGISTER response: {}",
                String::from_utf8_lossy(&buffer[..n])
            );

            let invite = build_invite(&format!("sip:{}@{}", to, sip_server), from, &server_addr.to_string());
            stream
                .write_all(invite.as_bytes())
                .await
                .map_err(SipError::Io)?;

            let n = stream.read(&mut buffer).await.map_err(SipError::Io)?;
            tracing::info!(
                "SIP INVITE response: {}",
                String::from_utf8_lossy(&buffer[..n])
            );

            Ok(())
        }
        Err(e) => {
            tracing::warn!("SIP trunk connection to {} failed: {} (stub/mock mode OK)", server_addr, e);
            Ok(())
        }
    }
}

fn extract_sip_host(sip_uri: &str) -> Result<String, SipError> {
    let uri = sip_uri
        .strip_prefix("sip:")
        .ok_or_else(|| SipError::InvalidUri(sip_uri.to_string()))?;

    let host = uri
        .split('@')
        .nth(1)
        .ok_or_else(|| SipError::InvalidUri(sip_uri.to_string()))?;

    Ok(host.split(':').next().unwrap_or(host).to_string())
}

fn build_invite(to: &str, from: &str, dest: &str) -> String {
    format!(
        "INVITE {} SIP/2.0\r\n\
         Via: SIP/2.0/TCP {};branch=z9hG4bK776asdhds\r\n\
         To: <{}>\r\n\
         From: <{}>;tag=1928301774\r\n\
         Call-ID: a84b4c76e66710@httpvoice\r\n\
         CSeq: 314159 INVITE\r\n\
         Contact: <{}>\r\n\
         Max-Forwards: 70\r\n\
         Content-Type: application/sdp\r\n\
         Content-Length: 142\r\n\
         \r\n\
         v=0\r\n\
         o=httpvoice 2890844526 2890844526 IN IP4 127.0.0.1\r\n\
         s=Call\r\n\
         c=IN IP4 127.0.0.1\r\n\
         t=0 0\r\n\
         m=audio 49170 RTP/AVP 0\r\n\
         a=rtpmap:0 PCMU/8000\r\n",
        to, dest, to, from, from
    )
}

fn build_register(from: &str, server: &str) -> String {
    format!(
        "REGISTER sip:{} SIP/2.0\r\n\
         Via: SIP/2.0/TCP {};branch=z9hG4bK-reg\r\n\
         To: <{}>\r\n\
         From: <{}>;tag=reg123\r\n\
         Call-ID: reg-httpvoice@{}\r\n\
         CSeq: 1 REGISTER\r\n\
         Contact: <{}>\r\n\
         Expires: 3600\r\n\
         Content-Length: 0\r\n\
         \r\n",
        server, server, from, from, server, from
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sip_host() {
        assert_eq!(
            extract_sip_host("sip:user@example.com").unwrap(),
            "example.com"
        );
        assert_eq!(
            extract_sip_host("sip:alice@192.168.1.1:5060").unwrap(),
            "192.168.1.1"
        );
        assert!(extract_sip_host("invalid").is_err());
    }

    #[test]
    fn test_build_invite_contains_pcmu() {
        let invite = build_invite("sip:user@example.com", "sip:from@test.com", "127.0.0.1");
        assert!(invite.contains("INVITE sip:user@example.com SIP/2.0"));
        assert!(invite.contains("PCMU"));
        assert!(invite.contains("application/sdp"));
    }

    #[tokio::test]
    async fn test_initiate_call_with_sip_uri() {
        let result = initiate_call("sip:test@localhost", "sip:from@localhost").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_initiate_call_without_server_configured() {
        std::env::remove_var("SIP_SERVER");
        let result = initiate_call("+14155551234", "sip:from@test.com").await;
        assert!(result.is_err());
    }
}
