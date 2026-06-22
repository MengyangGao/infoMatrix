use std::path::{Path, PathBuf};

use ::icon::extract_icon_candidates;
use sha2::{Digest, Sha256};
use shared_api::misc::hex_encode;
use storage::Storage;
use tracing::warn;
use url::Url;

use crate::config::AppContext;
use crate::state::{build_http_client, open_storage};

pub(crate) async fn discover_and_cache_site_icon(
    context: &AppContext,
    storage: &mut Storage,
    feed_id: &str,
    site_url: &Url,
) -> Option<Url> {
    let client = build_http_client(context).ok()?;
    let response = client.get(site_url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let final_url = response.url().clone();
    let html = response.text().await.ok()?;
    let candidates = extract_icon_candidates(&final_url, &html, None);
    let cache_dir = icon_cache_dir(&context.db_path);
    let _ = std::fs::create_dir_all(&cache_dir);

    for candidate in candidates {
        let response = client.get(candidate.url.clone()).send().await.ok()?;
        if !response.status().is_success() {
            continue;
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes = response.bytes().await.ok()?.to_vec();
        if bytes.len() > 512 * 1024 {
            continue;
        }
        if let Some(content_type_value) = content_type.as_deref() {
            if !content_type_value.to_ascii_lowercase().starts_with("image/") {
                continue;
            }
        }

        let sha256 = Sha256::digest(&bytes);
        let sha256_hex = hex_encode(sha256);
        let local_path = cache_dir.join(format!(
            "{}{}",
            sha256_hex,
            icon_file_extension(content_type.as_deref())
        ));
        if std::fs::write(&local_path, &bytes).is_err() {
            continue;
        }

        let _ = storage.set_feed_icon_asset(
            feed_id,
            candidate.url.as_str(),
            content_type.as_deref(),
            bytes.len() as i64,
            &sha256_hex,
            local_path.to_string_lossy().as_ref(),
        );
        return Some(candidate.url);
    }

    let fallback_url = final_url.join("/favicon.ico").ok()?;
    let response = client.get(fallback_url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response.bytes().await.ok()?.to_vec();
    if bytes.len() > 512 * 1024 {
        return None;
    }
    if let Some(content_type_value) = content_type.as_deref() {
        if !content_type_value.to_ascii_lowercase().starts_with("image/") {
            return None;
        }
    }

    let sha256 = Sha256::digest(&bytes);
    let sha256_hex = hex_encode(sha256);
    let local_path =
        cache_dir.join(format!("{}{}", sha256_hex, icon_file_extension(content_type.as_deref())));
    if std::fs::write(&local_path, &bytes).is_err() {
        return None;
    }

    let _ = storage.set_feed_icon_asset(
        feed_id,
        fallback_url.as_str(),
        content_type.as_deref(),
        bytes.len() as i64,
        &sha256_hex,
        local_path.to_string_lossy().as_ref(),
    );
    Some(fallback_url)
}

pub(crate) fn icon_cache_dir(db_path: &str) -> PathBuf {
    let db_path = Path::new(db_path);
    db_path
        .parent()
        .map(|parent| parent.join("icon-cache"))
        .unwrap_or_else(|| PathBuf::from("icon-cache"))
}

pub(crate) fn icon_file_extension(content_type: Option<&str>) -> &'static str {
    match content_type.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some(value) if value.contains("png") => ".png",
        Some(value) if value.contains("jpeg") || value.contains("jpg") => ".jpg",
        Some(value) if value.contains("webp") => ".webp",
        Some(value) if value.contains("svg") => ".svg",
        Some(value) if value.contains("x-icon") || value.contains("icon") => ".ico",
        _ => ".bin",
    }
}

pub(crate) fn spawn_background_icon_refresh(context: AppContext, feed_id: String, site_url: Url) {
    tokio::task::spawn_blocking(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
            warn!("background icon refresh skipped: failed to build runtime");
            return;
        };

        runtime.block_on(async move {
            let Ok(mut storage) = open_storage(&context) else {
                warn!("background icon refresh skipped: could not open storage");
                return;
            };
            let _ = discover_and_cache_site_icon(&context, &mut storage, &feed_id, &site_url).await;
        });
    });
}
