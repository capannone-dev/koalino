//! Guards the loopback HTTP/WebSocket API against cross-site browser attacks.
//!
//! The daemon has no user authentication: it trusts whoever can reach its loopback address.
//! Browsers break that assumption in two ways, which this middleware closes:
//!
//! - **Cross-site WebSocket hijacking.** CORS does not apply to WebSocket handshakes, so any page
//!   could open `/ws` and read live transcripts. Browsers always send `Origin` on cross-site
//!   requests, so a foreign `Origin` is refused on every route.
//! - **DNS rebinding.** A page served from `evil.example` can re-point that name at 127.0.0.1 and
//!   then make same-origin requests. Such requests carry `Host: evil.example`, so only `localhost`
//!   and IP literals (which a DNS answer cannot change) are accepted as hosts.
//!
//! Requests without `Origin` or `Host` come from non-browser clients (the CLI, scripts, Next.js
//! server components) and are served.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// The web dashboard's origin, the only browser origin allowed to call the API (also used by CORS).
pub const DASHBOARD_ORIGIN: &str = "http://localhost:3000";

/// Refuses requests whose `Host` or `Origin` shows a cross-site browser attack.
///
/// Inputs: The incoming request and the rest of the middleware stack.
/// Outputs: The downstream response, or `403 Forbidden` with no body details when the `Host` is a
/// DNS name other than `localhost`, or when `Origin` is present and is not `DASHBOARD_ORIGIN`.
/// Side effects: Logs refusals at WARN with the reason only; header values are not logged, since
/// they are attacker-controlled.
pub async fn reject_cross_site_requests(request: Request, next: Next) -> Response {
    if let Some(refusal_reason) = cross_site_refusal_reason(request.headers()) {
        tracing::warn!(reason = refusal_reason, "refused cross-site API request");
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

/// Explains why `headers` must be refused, or returns `None` when the request may proceed.
///
/// Inputs: The request headers. Outputs: A static reason string for logging, or `None`.
/// Side effects: None.
fn cross_site_refusal_reason(headers: &HeaderMap) -> Option<&'static str> {
    if let Some(host_value) = headers.get(header::HOST) {
        let is_trusted_host = host_value.to_str().is_ok_and(is_trusted_host);
        if !is_trusted_host {
            return Some("untrusted host");
        }
    }
    if let Some(origin_value) = headers.get(header::ORIGIN)
        && origin_value.as_bytes() != DASHBOARD_ORIGIN.as_bytes()
    {
        return Some("foreign origin");
    }
    None
}

/// Reports whether a `Host` header value names `localhost` or an IP literal, with any port.
///
/// Inputs: The raw header value, e.g. `localhost:8080`, `127.0.0.1`, or `[::1]:8080`.
/// Outputs: `true` for those forms, `false` for any other DNS name or a malformed value.
/// Side effects: None.
fn is_trusted_host(host_value: &str) -> bool {
    let host_name = if let Some(bracketed) = host_value.strip_prefix('[') {
        // IPv6 literal: `[addr]` or `[addr]:port`.
        match bracketed.split_once(']') {
            Some((ipv6_literal, port_suffix))
                if port_suffix.is_empty()
                    || port_suffix
                        .strip_prefix(':')
                        .is_some_and(|port| port.parse::<u16>().is_ok()) =>
            {
                return ipv6_literal.parse::<std::net::Ipv6Addr>().is_ok();
            }
            _ => return false,
        }
    } else {
        match host_value.rsplit_once(':') {
            Some((name, port)) if port.parse::<u16>().is_ok() => name,
            Some(_) => return false,
            None => host_value,
        }
    };
    host_name.eq_ignore_ascii_case("localhost") || host_name.parse::<std::net::Ipv4Addr>().is_ok()
}
