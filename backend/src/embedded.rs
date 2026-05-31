use axum::response::{IntoResponse, Response};
use axum::http::{header, StatusCode, Uri};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct Frontend;

pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Frontend::get(path) {
        let mime = mime_type(path);
        return Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap();
    }

    if let Some(file) = Frontend::get("index.html") {
        return Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(axum::body::Body::from(file.data.to_vec()))
            .unwrap();
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn mime_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}
