use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use models::{EntryKind, EntrySource, EntrySourceKind, ItemScopeCounts, ItemStatePatch, NewEntry};
use sha2::{Digest, Sha256};
use shared_api::labels::{entry_kind_label, entry_source_kind_label, parse_item_filter};
use shared_api::misc::hex_encode;
use shared_api::url::enforce_web_url;
use tracing::warn;
use url::Url;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::services::webpage::{
    ExtractedContent, capture_webpage_snapshot, clean_preview_text, extract_full_content,
    looks_like_markdown, markdown_to_safe_html, prepare_display_html, sanitize_html_fragment,
    webpage_fallback_title,
};
use crate::state::{build_http_client, open_app_core, open_storage};
use crate::views::{
    CreateEntryRequest, FullTextResponse, ItemDetailView, ItemView, ListAllItemsQuery,
    ListItemsQuery, PatchItemStateRequest, PatchItemStateResponse,
};

pub(crate) async fn list_items(
    State(context): State<AppContext>,
    AxumPath(feed_id): AxumPath<String>,
    Query(query): Query<ListItemsQuery>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let core = open_app_core(&context)?;
    let items = core
        .search_items_for_feed(&feed_id, query.limit.unwrap_or(100), query.q.as_deref())?
        .into_iter()
        .map(item_to_view)
        .collect();
    Ok(Json(items))
}

pub(crate) async fn list_all_items(
    State(context): State<AppContext>,
    Query(query): Query<ListAllItemsQuery>,
) -> Result<Json<Vec<ItemView>>, ApiError> {
    let core = open_app_core(&context)?;
    let filter = parse_item_filter(query.filter.as_deref());
    let items = core
        .search_all_items(query.limit.unwrap_or(200), query.q.as_deref(), filter, query.kind)?
        .into_iter()
        .map(item_to_view)
        .collect();
    Ok(Json(items))
}

pub(crate) async fn create_entry(
    State(context): State<AppContext>,
    Json(payload): Json<CreateEntryRequest>,
) -> Result<Json<ItemDetailView>, ApiError> {
    let entry_id = payload.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let source_url = payload
        .source_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| ApiError::BadRequest(err.to_string())))
        .transpose()?;
    let kind = payload.kind.unwrap_or_else(|| {
        if source_url.is_some() { EntryKind::Bookmark } else { EntryKind::Note }
    });
    let source_kind = payload.source_kind.unwrap_or_else(|| {
        if source_url.is_some() { EntrySourceKind::Web } else { EntrySourceKind::Manual }
    });
    let is_web_capture = source_url.is_some() || matches!(kind, EntryKind::Bookmark);
    let client = if is_web_capture { Some(build_http_client(&context)?) } else { None };
    let fallback_source_title = source_url.as_ref().and_then(webpage_fallback_title);
    let webpage_capture = if let (Some(client), Some(url)) = (client.as_ref(), source_url.as_ref())
    {
        match capture_webpage_snapshot(client, url).await {
            Ok(capture) => Some(capture),
            Err(err) => {
                warn!("webpage capture failed for {}: {}", url, err);
                None
            }
        }
    } else {
        None
    };
    let resolved_title = {
        let requested = payload.title.trim();
        if !requested.is_empty() {
            requested.to_owned()
        } else if let Some(capture) = webpage_capture.as_ref() {
            capture
                .title
                .clone()
                .or(fallback_source_title.clone())
                .unwrap_or_else(|| "Untitled".to_owned())
        } else if let Some(url) = source_url.as_ref() {
            webpage_fallback_title(url).unwrap_or_else(|| "Untitled".to_owned())
        } else {
            "未命名随想".to_owned()
        }
    };
    let canonical_url = payload
        .canonical_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| ApiError::BadRequest(err.to_string())))
        .transpose()?
        .or_else(|| webpage_capture.as_ref().map(|capture| capture.final_url.clone()))
        .or_else(|| source_url.clone());
    let content_html = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_html.clone())
        .or(payload.content_html);
    let content_text = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_text.clone())
        .or(payload.content_text);
    let raw_hash = payload.raw_hash.unwrap_or_else(|| {
        let mut hasher = Sha256::new();
        hasher.update(entry_kind_label(kind));
        hasher.update(resolved_title.as_bytes());
        if let Some(value) = canonical_url.as_ref() {
            hasher.update(value.as_str().as_bytes());
        }
        if let Some(value) = source_url.as_ref() {
            hasher.update(value.as_str().as_bytes());
        }
        if let Some(value) = payload.summary.as_deref() {
            hasher.update(value.as_bytes());
        }
        if let Some(value) = content_text.as_deref() {
            hasher.update(value.as_bytes());
        }
        hex_encode(hasher.finalize())
    });

    let mut core = open_app_core(&context)?;
    core.create_entry(NewEntry {
        id: Some(entry_id.clone()),
        kind,
        source: EntrySource {
            source_kind,
            source_id: payload.source_id,
            source_url,
            source_title: webpage_capture
                .as_ref()
                .and_then(|capture| capture.title.clone())
                .or(payload.source_title)
                .or(fallback_source_title),
        },
        external_item_id: None,
        canonical_url,
        title: resolved_title,
        author: None,
        summary: payload.summary,
        content_html,
        content_text,
        published_at: None,
        updated_at: None,
        raw_hash,
        dedup_reason: None,
        duplicate_of_entry_id: None,
    })?;

    let detail = core.item_detail(&entry_id)?;
    let prepared_html = prepare_display_html(
        detail.content_html.as_deref(),
        detail.content_text.as_deref(),
        detail.summary.as_deref(),
    );
    Ok(Json(ItemDetailView {
        id: detail.id,
        kind: entry_kind_label(detail.kind).to_owned(),
        source_kind: entry_source_kind_label(detail.source_kind).to_owned(),
        source_id: detail.source_id,
        source_url: detail.source_url.map(|url| url.to_string()),
        title: detail.title,
        canonical_url: detail.canonical_url.map(|url| url.to_string()),
        published_at: detail.published_at,
        summary: detail.summary,
        content_html: prepared_html,
        content_text: detail.content_text,
        is_read: detail.is_read,
        is_starred: detail.is_starred,
        is_saved_for_later: detail.is_saved_for_later,
        is_archived: detail.is_archived,
    }))
}

pub(crate) async fn item_counts(
    State(context): State<AppContext>,
) -> Result<Json<ItemScopeCounts>, ApiError> {
    let core = open_app_core(&context)?;
    Ok(Json(core.item_counts()?))
}

pub(crate) async fn get_item_detail(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
) -> Result<Json<ItemDetailView>, ApiError> {
    let core = open_app_core(&context)?;
    let detail = core.item_detail(&item_id)?;
    let prepared_html = prepare_display_html(
        detail.content_html.as_deref(),
        detail.content_text.as_deref(),
        detail.summary.as_deref(),
    );
    Ok(Json(ItemDetailView {
        id: detail.id,
        kind: entry_kind_label(detail.kind).to_owned(),
        source_kind: entry_source_kind_label(detail.source_kind).to_owned(),
        source_id: detail.source_id,
        source_url: detail.source_url.map(|url| url.to_string()),
        title: detail.title,
        canonical_url: detail.canonical_url.map(|url| url.to_string()),
        published_at: detail.published_at,
        summary: detail.summary,
        content_html: prepared_html,
        content_text: detail.content_text,
        is_read: detail.is_read,
        is_starred: detail.is_starred,
        is_saved_for_later: detail.is_saved_for_later,
        is_archived: detail.is_archived,
    }))
}

pub(crate) async fn fetch_item_fulltext(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
) -> Result<Json<FullTextResponse>, ApiError> {
    let mut storage = open_storage(&context)?;
    let detail = storage.get_item_detail(&item_id)?;

    let canonical_url = detail
        .canonical_url
        .ok_or_else(|| ApiError::BadRequest("item has no canonical url for fulltext".to_owned()))?;
    enforce_web_url(&canonical_url, "article url")?;

    let client = build_http_client(&context)?;
    let response = client
        .get(canonical_url.clone())
        .send()
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    if !response.status().is_success() {
        return Err(ApiError::BadRequest(format!(
            "fulltext fetch failed with HTTP {}",
            response.status().as_u16()
        )));
    }

    let html = response.text().await.map_err(|err| ApiError::Internal(err.to_string()))?;
    let extracted = extract_full_content(&html)
        .or_else(|| {
            detail.content_text.as_deref().and_then(|source_text| {
                if looks_like_markdown(source_text) {
                    let rendered_html = markdown_to_safe_html(source_text);
                    Some(ExtractedContent {
                        content_html: Some(rendered_html),
                        content_text: source_text.to_owned(),
                        source: "feed_markdown",
                    })
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| {
            ApiError::BadRequest(
                "unable to extract meaningful fulltext from article html".to_owned(),
            )
        })?;
    let sanitized_html = extracted.content_html.as_deref().map(sanitize_html_fragment);

    storage.upsert_item_content(
        &item_id,
        sanitized_html.as_deref(),
        Some(&extracted.content_text),
    )?;

    Ok(Json(FullTextResponse {
        item_id,
        content_text: extracted.content_text,
        source: extracted.source.to_owned(),
    }))
}

pub(crate) async fn patch_item_state(
    State(context): State<AppContext>,
    AxumPath(item_id): AxumPath<String>,
    Json(payload): Json<PatchItemStateRequest>,
) -> Result<Json<PatchItemStateResponse>, ApiError> {
    let core = open_app_core(&context)?;
    let state = core.patch_item_state(
        &item_id,
        &ItemStatePatch {
            is_read: payload.is_read,
            is_starred: payload.is_starred,
            is_saved_for_later: payload.is_saved_for_later,
            is_archived: payload.is_archived,
        },
    )?;

    Ok(Json(PatchItemStateResponse {
        item_id: state.item_id,
        is_read: state.is_read,
        is_starred: state.is_starred,
        is_saved_for_later: state.is_saved_for_later,
        is_archived: state.is_archived,
    }))
}

pub(crate) fn item_to_view(item: storage::ItemSummaryRow) -> ItemView {
    ItemView {
        id: item.id,
        kind: entry_kind_label(item.kind).to_owned(),
        source_kind: entry_source_kind_label(item.source_kind).to_owned(),
        source_id: item.source_id,
        source_url: item.source_url.map(|url| url.to_string()),
        title: item.title,
        canonical_url: item.canonical_url.map(|url| url.to_string()),
        published_at: item.published_at,
        summary_preview: item.summary_preview.as_deref().and_then(clean_preview_text),
        is_read: item.is_read,
        is_starred: item.is_starred,
        is_saved_for_later: item.is_saved_for_later,
        is_archived: item.is_archived,
    }
}
