use fetcher::{FeedHttpClient, FetchRequest, ReqwestFeedClient};
use models::ItemStatePatch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared_api::constants::{DEFAULT_TIMEOUT_SECS, USER_AGENT};
use shared_api::labels::{entry_kind_label, entry_source_kind_label};
use shared_api::misc::hex_encode;
use shared_api::url::enforce_web_url;
use shared_api::webpage::{extract_full_content, sanitize_html_fragment};
use url::Url;

use crate::TOKIO_RUNTIME;
use crate::content::capture_webpage_snapshot;
use crate::labels;
use crate::storage::open_app_core;

#[derive(Debug, Deserialize)]
pub struct ListItemsInput {
    pub db_path: Option<String>,
    pub feed_id: String,
    pub limit: Option<usize>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListEntriesInput {
    pub db_path: Option<String>,
    pub feed_id: Option<String>,
    pub filter: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchItemStateInput {
    pub db_path: Option<String>,
    pub item_id: String,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub is_saved_for_later: Option<bool>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntryInput {
    pub db_path: Option<String>,
    pub id: Option<String>,
    pub title: String,
    pub kind: Option<String>,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub source_title: Option<String>,
    pub canonical_url: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub raw_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FullTextInput {
    pub db_path: Option<String>,
    pub item_id: String,
}

#[derive(Debug, Serialize)]
pub struct EntryOutput {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub source_title: Option<String>,
    pub canonical_url: Option<String>,
    pub published_at: Option<String>,
    pub summary_preview: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_saved_for_later: bool,
    pub is_archived: bool,
}

#[derive(Debug, Serialize)]
pub struct PatchItemStateOutput {
    pub item_id: String,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_saved_for_later: bool,
    pub is_archived: bool,
}

#[derive(Debug, Serialize)]
pub struct FullTextOutput {
    pub item_id: String,
    pub content_text: String,
    pub source: String,
}

pub fn list_items(
    db_path: &Option<String>,
    feed_id: &str,
    limit: Option<usize>,
    q: Option<&str>,
) -> Result<Vec<EntryOutput>, String> {
    let core = open_app_core(db_path)?;
    let rows = core
        .search_items_for_feed(feed_id, limit.unwrap_or(100), q)
        .map_err(|err| err.to_string())?;

    let items: Vec<EntryOutput> = rows.into_iter().map(entry_summary_output_from_row).collect();

    Ok(items)
}

pub fn list_entries(
    db_path: &Option<String>,
    feed_id: Option<&str>,
    filter: Option<&str>,
    q: Option<&str>,
    limit: Option<usize>,
    kind: Option<&str>,
) -> Result<Vec<EntryOutput>, String> {
    let core = open_app_core(db_path)?;
    let filter = shared_api::labels::parse_item_filter(filter);
    let kind = kind.map(labels::parse_entry_kind_label);
    let rows = if let Some(feed_id) = feed_id {
        core.search_items_for_feed(feed_id, limit.unwrap_or(200), q)
            .map_err(|err| err.to_string())?
    } else {
        core.search_all_items(limit.unwrap_or(200), q, filter, kind)
            .map_err(|err| err.to_string())?
    };

    let items: Vec<EntryOutput> = rows.into_iter().map(entry_summary_output_from_row).collect();

    Ok(items)
}

pub fn item_counts(db_path: &Option<String>) -> Result<models::ItemScopeCounts, String> {
    let core = open_app_core(db_path)?;
    core.item_counts().map_err(|err| err.to_string())
}

pub fn get_entry(db_path: &Option<String>, item_id: &str) -> Result<EntryOutput, String> {
    let core = open_app_core(db_path)?;
    let detail = core.item_detail(item_id).map_err(|err| err.to_string())?;
    Ok(entry_detail_output_from_row(detail, None, None))
}

pub fn create_entry(
    db_path: &Option<String>,
    payload: CreateEntryInput,
) -> Result<EntryOutput, String> {
    let mut core = open_app_core(db_path)?;
    let entry_id = payload.id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let source_url = payload
        .source_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| format!("invalid source_url: {err}")))
        .transpose()?;
    let canonical_url = payload
        .canonical_url
        .as_deref()
        .map(|value| Url::parse(value).map_err(|err| format!("invalid canonical_url: {err}")))
        .transpose()?;
    let kind = payload.kind.as_deref().map(labels::parse_entry_kind_label).unwrap_or_else(|| {
        if source_url.is_some() { models::EntryKind::Bookmark } else { models::EntryKind::Note }
    });
    let source_kind =
        payload.source_kind.as_deref().map(labels::parse_entry_source_kind_label).unwrap_or_else(
            || {
                if source_url.is_some() {
                    models::EntrySourceKind::Web
                } else {
                    models::EntrySourceKind::Manual
                }
            },
        );
    let is_web_capture = source_url.is_some() || matches!(kind, models::EntryKind::Bookmark);
    let runtime = &*TOKIO_RUNTIME;
    let fallback_source_title =
        source_url.as_ref().and_then(shared_api::webpage::webpage_fallback_title);
    let webpage_capture = if is_web_capture {
        if let Some(source_url) = source_url.as_ref() {
            capture_webpage_snapshot(runtime, source_url).ok()
        } else {
            None
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
        } else if let Some(source_url) = source_url.as_ref() {
            shared_api::webpage::webpage_fallback_title(source_url)
                .unwrap_or_else(|| "Untitled".to_owned())
        } else {
            "未命名随想".to_owned()
        }
    };
    let content_html = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_html.clone())
        .or(payload.content_html);
    let content_text = webpage_capture
        .as_ref()
        .and_then(|capture| capture.content_text.clone())
        .or(payload.content_text);
    let canonical_url = canonical_url
        .or_else(|| webpage_capture.as_ref().map(|capture| capture.final_url.clone()))
        .or_else(|| source_url.clone());
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

    core.create_entry(models::NewEntry {
        id: Some(entry_id.clone()),
        kind,
        source: models::EntrySource {
            source_kind,
            source_id: payload.source_id,
            source_url,
            source_title: payload.source_title,
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
    })
    .map_err(|err| err.to_string())?;

    let detail = core.item_detail(&entry_id).map_err(|err| err.to_string())?;
    Ok(entry_detail_output_from_row(detail, None, None))
}

pub fn fetch_fulltext(db_path: &Option<String>, item_id: &str) -> Result<FullTextOutput, String> {
    let mut storage = crate::storage::open_storage(db_path)?;
    let detail = storage.get_item_detail(item_id).map_err(|err| err.to_string())?;

    let candidate_url = detail
        .canonical_url
        .or(detail.source_url)
        .ok_or_else(|| "entry has no source url for fulltext".to_owned())?;
    enforce_web_url(&candidate_url, "entry url").map_err(|err| err.to_string())?;

    let runtime = &*TOKIO_RUNTIME;
    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let request = FetchRequest { url: candidate_url.clone(), etag: None, last_modified: None };
    let response = runtime.block_on(client.fetch(&request)).map_err(|err| err.to_string())?;
    if response.status >= 400 {
        return Err(format!("fulltext fetch failed with HTTP {}", response.status));
    }

    let html = String::from_utf8(response.body).map_err(|err| err.to_string())?;
    let extracted = extract_full_content(&html)
        .ok_or_else(|| "unable to extract meaningful fulltext from entry html".to_owned())?;
    let sanitized_html = extracted.content_html.as_deref().map(sanitize_html_fragment);
    storage
        .upsert_item_content(item_id, sanitized_html.as_deref(), Some(&extracted.content_text))
        .map_err(|err| err.to_string())?;

    Ok(FullTextOutput {
        item_id: item_id.to_owned(),
        content_text: extracted.content_text,
        source: extracted.source.to_owned(),
    })
}

pub fn patch_item_state(
    db_path: &Option<String>,
    item_id: &str,
    is_read: Option<bool>,
    is_starred: Option<bool>,
    is_saved_for_later: Option<bool>,
    is_archived: Option<bool>,
) -> Result<PatchItemStateOutput, String> {
    let core = open_app_core(db_path)?;
    let state = core
        .patch_item_state(
            item_id,
            &ItemStatePatch { is_read, is_starred, is_saved_for_later, is_archived },
        )
        .map_err(|err| err.to_string())?;

    Ok(PatchItemStateOutput {
        item_id: state.item_id,
        is_read: state.is_read,
        is_starred: state.is_starred,
        is_saved_for_later: state.is_saved_for_later,
        is_archived: state.is_archived,
    })
}

fn entry_summary_output_from_row(row: storage::ItemSummaryRow) -> EntryOutput {
    EntryOutput {
        id: row.id,
        title: row.title,
        kind: entry_kind_label(row.kind).to_owned(),
        source_kind: entry_source_kind_label(row.source_kind).to_owned(),
        source_id: row.source_id,
        source_url: row.source_url.map(|value| value.to_string()),
        source_title: None,
        canonical_url: row.canonical_url.map(|value| value.to_string()),
        published_at: row.published_at,
        summary_preview: row.summary_preview,
        summary: None,
        content_html: None,
        content_text: None,
        is_read: row.is_read,
        is_starred: row.is_starred,
        is_saved_for_later: row.is_saved_for_later,
        is_archived: row.is_archived,
    }
}

fn entry_detail_output_from_row(
    row: storage::ItemDetailRow,
    summary_preview: Option<String>,
    content_html_override: Option<String>,
) -> EntryOutput {
    EntryOutput {
        id: row.id,
        title: row.title,
        kind: entry_kind_label(row.kind).to_owned(),
        source_kind: entry_source_kind_label(row.source_kind).to_owned(),
        source_id: row.source_id,
        source_url: row.source_url.map(|value| value.to_string()),
        source_title: None,
        canonical_url: row.canonical_url.map(|value| value.to_string()),
        published_at: row.published_at,
        summary_preview,
        summary: row.summary,
        content_html: content_html_override.or(row.content_html),
        content_text: row.content_text,
        is_read: row.is_read,
        is_starred: row.is_starred,
        is_saved_for_later: row.is_saved_for_later,
        is_archived: row.is_archived,
    }
}
