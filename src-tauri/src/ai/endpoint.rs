//! `local_only` enforcement.
//!
//! When `security.local_only` is on (the default), OMNIX may only talk to:
//!
//! * the Ollama provider (cloud providers are refused outright), and
//! * hosts whose **every resolved address** is loopback, RFC 1918 private,
//!   link-local, CGNAT `100.64.0.0/10` (Tailscale) or IPv6 ULA `fc00::/7`
//!   (includes Tailscale's `fd7a:115c:a1e0::/48`).
//!
//! This is the control that keeps prompts, which may contain PHI on
//! healthcare-network machines, from leaving the local network. It is enforced
//! in Rust, so the webview cannot bypass it (the CSP also blocks the webview
//! from making its own network requests).
//!
//! Residual risk: DNS may change between this check and the request (DNS
//! rebinding). Prefer IP literals or `localhost` for sensitive deployments.

use crate::error::{AppError, AppResult};
use reqwest::Url;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Providers that never leave the machine / LAN.
pub const LOCAL_PROVIDERS: &[&str] = &["ollama"];

/// Refuse cloud providers while `local_only` is on.
pub fn ensure_provider_allowed(provider: &str, local_only: bool) -> AppResult<()> {
    if local_only && !LOCAL_PROVIDERS.contains(&provider) {
        return Err(AppError::LocalOnly(format!(
            "provider `{provider}` is a cloud service; turn off local-only mode in Settings → Security to use it"
        )));
    }
    Ok(())
}

/// Verify `url` points at a local/private host when `local_only` is on.
pub async fn ensure_endpoint_allowed(url: &str, local_only: bool) -> AppResult<Url> {
    let u =
        Url::parse(url).map_err(|e| AppError::InvalidInput(format!("invalid URL `{url}`: {e}")))?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput(format!(
            "unsupported URL scheme `{}`",
            u.scheme()
        )));
    }
    if !local_only {
        return Ok(u);
    }
    let port = u.port_or_known_default().unwrap_or(80);
    let ips: Vec<IpAddr> = match u.host() {
        Some(url::Host::Ipv4(ip)) => vec![IpAddr::V4(ip)],
        Some(url::Host::Ipv6(ip)) => vec![IpAddr::V6(ip)],
        Some(url::Host::Domain(d)) if d.eq_ignore_ascii_case("localhost") => {
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
        }
        Some(url::Host::Domain(d)) => tokio::net::lookup_host((d, port))
            .await
            .map_err(|e| AppError::Unavailable(format!("cannot resolve `{d}`: {e}")))?
            .map(|sa| sa.ip())
            .collect(),
        None => return Err(AppError::InvalidInput("URL has no host".into())),
    };
    if ips.is_empty() || !ips.iter().all(is_local_ip) {
        return Err(AppError::LocalOnly(format!(
            "`{}` is not a local or private-network address",
            u.host_str().unwrap_or_default()
        )));
    }
    Ok(u)
}

/// Loopback, private, link-local, CGNAT/Tailscale or IPv6 ULA.
pub fn is_local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_local_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_local_v4(&v4);
            }
            v6.is_loopback() || is_ula(v6) || is_link_local_v6(v6)
        }
    }
}

fn is_local_v4(ip: &Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        // 100.64.0.0/10 carrier-grade NAT, used by Tailscale.
        || (a == 100 && (64..=127).contains(&b))
}

fn is_ula(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_link_local_v6(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_ips() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.10",
            "100.101.102.103",
            "169.254.1.1",
            "::1",
            "fd7a:115c:a1e0::1",
            "fe80::1",
            "::ffff:192.168.1.1",
        ] {
            assert!(is_local_ip(&ip.parse().expect("ip")), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "100.128.0.1",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(!is_local_ip(&ip.parse().expect("ip")), "{ip}");
        }
    }

    #[tokio::test]
    async fn local_only_blocks_public_endpoints() {
        assert!(ensure_endpoint_allowed("http://localhost:11434", true)
            .await
            .is_ok());
        assert!(ensure_endpoint_allowed("http://127.0.0.1:11434", true)
            .await
            .is_ok());
        assert!(ensure_endpoint_allowed("http://[::1]:11434", true)
            .await
            .is_ok());
        assert!(ensure_endpoint_allowed("http://100.100.1.2:11434", true)
            .await
            .is_ok());
        assert!(matches!(
            ensure_endpoint_allowed("https://8.8.8.8", true).await,
            Err(AppError::LocalOnly(_))
        ));
        assert!(ensure_endpoint_allowed("https://8.8.8.8", false)
            .await
            .is_ok());
        assert!(ensure_endpoint_allowed("file:///etc/passwd", false)
            .await
            .is_err());
    }

    #[test]
    fn cloud_providers_blocked_when_local_only() {
        assert!(ensure_provider_allowed("ollama", true).is_ok());
        assert!(matches!(
            ensure_provider_allowed("anthropic", true),
            Err(AppError::LocalOnly(_))
        ));
        assert!(ensure_provider_allowed("anthropic", false).is_ok());
    }
}
