//! Defines the cross-site request guard contract: DNS-rebinding hosts and foreign browser origins
//! are refused before any handler runs, while local and non-browser clients keep working.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::api_tests::seeded_router_for_guard_tests;

/// Sends `GET path` with the given optional `Host` and `Origin` headers and returns the status.
async fn status_for(path: &str, host: Option<&str>, origin: Option<&str>) -> StatusCode {
    let mut request_builder = Request::builder().uri(path);
    if let Some(host_value) = host {
        request_builder = request_builder.header("host", host_value);
    }
    if let Some(origin_value) = origin {
        request_builder = request_builder.header("origin", origin_value);
    }
    let request = request_builder
        .body(Body::empty())
        .unwrap_or_else(|error| panic!("test request must build: {error}"));
    seeded_router_for_guard_tests()
        .oneshot(request)
        .await
        .unwrap_or_else(|error| panic!("router must answer: {error}"))
        .status()
}

/// A rebound DNS name pointing at 127.0.0.1 must not reach meeting data.
#[tokio::test]
async fn rejects_a_host_header_with_a_foreign_dns_name() {
    for host in [
        "evil.example",
        "evil.example:8080",
        "localhost.evil.example:8080",
    ] {
        assert_eq!(
            status_for("/api/meetings", Some(host), None).await,
            StatusCode::FORBIDDEN,
            "{host}"
        );
    }
}

/// Loopback names and IP literals (which DNS rebinding cannot produce) are served.
#[tokio::test]
async fn accepts_localhost_and_ip_literal_hosts() {
    for host in [
        "localhost",
        "localhost:8080",
        "LOCALHOST:8080",
        "127.0.0.1:8080",
        "[::1]:8080",
        "192.168.1.20:8080",
    ] {
        assert_eq!(
            status_for("/api/meetings", Some(host), None).await,
            StatusCode::OK,
            "{host}"
        );
    }
}

/// A page on another site must not read the API or open the live-transcript WebSocket, which
/// CORS does not protect.
#[tokio::test]
async fn rejects_requests_from_a_foreign_browser_origin() {
    for path in ["/api/meetings", "/ws"] {
        assert_eq!(
            status_for(path, Some("127.0.0.1:8080"), Some("https://evil.example")).await,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert_eq!(
        status_for("/api/meetings", Some("127.0.0.1:8080"), Some("null")).await,
        StatusCode::FORBIDDEN
    );
}

/// The dashboard origin and clients that send no `Origin` (CLI tools, server-side fetches) work.
#[tokio::test]
async fn accepts_the_dashboard_origin_and_requests_without_origin() {
    assert_eq!(
        status_for(
            "/api/meetings",
            Some("127.0.0.1:8080"),
            Some("http://localhost:3000")
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        status_for("/api/meetings", None, None).await,
        StatusCode::OK
    );
}
