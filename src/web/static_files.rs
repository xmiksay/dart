//! Single-binary SPA serving via `rust-embed`. In release builds the contents of
//! `web/dist` are embedded into the binary at compile time; in debug builds
//! rust-embed reads them from disk at runtime, so the backend still builds and
//! runs when `web/dist` is absent (the frontend is built separately).

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

/// Axum fallback: serve the embedded asset matching the request path; for any
/// other non-`/api`, non-`/ws` route serve the SPA shell (`index.html`) so the
/// client-side router can take over. Genuinely missing asset-looking paths 404.
pub async fn static_handler(uri: Uri) -> Response {
    serve(uri.path().trim_start_matches('/'))
}

fn serve(path: &str) -> Response {
    // Unmatched API/WS paths must 404, never fall through to the SPA shell.
    if path == "api" || path == "ws" || path.starts_with("api/") || path.starts_with("ws/") {
        return not_found();
    }

    if path.is_empty() {
        return index_html();
    }

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        // A path with a file extension is an asset request: a miss is a real 404.
        // Extensionless paths are client routes, so serve the SPA shell.
        None if looks_like_asset(path) => not_found(),
        None => index_html(),
    }
}

fn index_html() -> Response {
    match Assets::get("index.html") {
        Some(content) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            content.data,
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "frontend not built (web/dist missing); API is available under /api",
        )
            .into_response(),
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

/// A request targets a static asset (rather than a client route) when its final
/// path segment carries a file extension, e.g. `assets/index-abc.js`.
fn looks_like_asset(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|seg| seg.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_paths_have_extensions() {
        assert!(looks_like_asset("assets/index-abc123.js"));
        assert!(looks_like_asset("favicon.ico"));
        assert!(looks_like_asset("assets/logo.svg"));
    }

    #[test]
    fn client_routes_have_no_extension() {
        assert!(!looks_like_asset("games"));
        assert!(!looks_like_asset("games/42"));
        assert!(!looks_like_asset("setup/players"));
        assert!(!looks_like_asset(""));
    }

    #[test]
    fn unmatched_api_and_ws_paths_404() {
        assert_eq!(serve("api/unknown").status(), StatusCode::NOT_FOUND);
        assert_eq!(serve("api").status(), StatusCode::NOT_FOUND);
        assert_eq!(serve("ws").status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn missing_asset_404s() {
        // No build emits this name, so it misses whether or not web/dist exists.
        assert_eq!(
            serve("assets/does-not-exist.js").status(),
            StatusCode::NOT_FOUND
        );
    }
}
