use std::path::PathBuf;

pub struct AssetReply {
    pub status: u16,
    pub mime: &'static str,
    pub body: Vec<u8>,
}

pub fn serve_asset(uri_path: &str) -> AssetReply {
    let path_str = uri_path.strip_prefix('/').unwrap_or(uri_path);
    let decoded = urlencoding::decode(path_str).unwrap_or(std::borrow::Cow::Borrowed(path_str));
    let mut path = PathBuf::from(decoded.as_ref());

    if let Ok(p) = path.strip_prefix("//") {
        path = PathBuf::from(format!("/{}", p.display()));
    }

    #[cfg(unix)]
    if !path.is_absolute() && decoded.starts_with('/') {
        path = PathBuf::from(decoded.as_ref());
    }

    let is_safe = path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == "games" || s == "cache" || s == "templates" || s == "loaders"
    });

    if is_safe && path.exists() && path.is_file() {
        let content = std::fs::read(&path).unwrap_or_default();
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime = match extension.to_lowercase().as_str() {
            "png" => "image/png",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "jpg" | "jpeg" => "image/jpeg",
            _ => "application/octet-stream",
        };
        AssetReply {
            status: 200,
            mime,
            body: content,
        }
    } else {
        AssetReply {
            status: 403,
            mime: "text/plain",
            body: Vec::new(),
        }
    }
}
