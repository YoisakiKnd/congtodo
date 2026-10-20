//! 前端静态资源：release 构建时把 web/dist 打进二进制，debug 构建时直接读磁盘。

use axum::{
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../web/dist"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (path, file) = match Assets::get(path) {
        Some(f) if !path.is_empty() => (path, f),
        // SPA 回退
        _ => match Assets::get("index.html") {
            Some(f) => ("index.html", f),
            None => return (StatusCode::NOT_FOUND, "frontend not built: run `npm run build` in web/").into_response(),
        },
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite 产物带 hash，可以长缓存
    let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    (
        [(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, cache)],
        file.data,
    )
        .into_response()
}
