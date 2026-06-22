mod config;
mod error;
mod handlers;
mod routes;
mod services;
mod state;
mod views;

#[allow(unused_imports)]
pub(crate) use crate::config::{
    AppContext, RuntimeConfig, resolve_runtime_config, resolve_runtime_config_with_env,
};
#[allow(unused_imports)]
pub(crate) use crate::routes::app_router;
#[allow(unused_imports)]
pub(crate) use crate::services::discovery::{
    candidate_site_urls, fallback_candidate_urls, feed_candidate_score,
};
#[allow(unused_imports)]
pub(crate) use crate::services::webpage::{
    clean_preview_text, extract_full_content, prepare_display_html,
};
#[allow(unused_imports)]
pub(crate) use shared_api::labels::confidence_from_score;

use std::error::Error;

use shared_api::db::ensure_parent_dir;
use storage::Storage;
use tracing::info;

use crate::services::refresh::spawn_background_refresh_task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = resolve_runtime_config(std::env::args().skip(1))?;
    ensure_parent_dir(&config.db_path)?;

    let storage = Storage::open(&config.db_path)?;
    storage.migrate()?;

    let context = AppContext {
        db_path: config.db_path,
        user_agent: "InfoMatrix/0.1 (+https://github.com/MengyangGao/infoMatrix)".to_owned(),
        timeout_secs: 20,
        migrate_on_open: false,
        api_token: config.api_token,
    };
    spawn_background_refresh_task(context.clone());
    let app = app_router(context);

    info!("infomatrix app server listening on http://{}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::response::Html;
    use axum::routing::get;
    use chrono::Utc;
    use http_body_util::BodyExt;
    use models::{
        DigestPolicy, FeedType, GlobalNotificationSettings, ItemStatePatch, NewFeed,
        NormalizedItem, NotificationDeliveryState, NotificationEvent, NotificationMode,
        NotificationSettings, QuietHours,
    };
    use serde_json::json;
    use storage::Storage;
    use tower::ServiceExt;
    use url::Url;

    use super::{
        AppContext, app_router, candidate_site_urls, clean_preview_text, confidence_from_score,
        extract_full_content, fallback_candidate_urls, feed_candidate_score, prepare_display_html,
        resolve_runtime_config, resolve_runtime_config_with_env,
    };

    fn sample_item(feed_id: &str, item_id: &str) -> NormalizedItem {
        NormalizedItem {
            id: item_id.to_owned(),
            source_feed_id: feed_id.to_owned(),
            external_item_id: Some("ext-1".to_owned()),
            canonical_url: Some(Url::parse("https://example.com/post").expect("url parse")),
            title: "Sample Item".to_owned(),
            author: Some("Author".to_owned()),
            summary: Some("Summary".to_owned()),
            content_html: Some("<p>content</p>".to_owned()),
            content_text: Some("content".to_owned()),
            published_at: None,
            updated_at: None,
            raw_hash: "abc".to_owned(),
            dedup_reason: None,
            duplicate_of_item_id: None,
        }
    }

    async fn spawn_webpage_server(html: &'static str) -> Url {
        let app = axum::Router::new().route("/", get(move || async move { Html(html) }));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind test webpage server");
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test webpage");
        });
        Url::parse(&format!("http://{addr}/")).expect("test webpage url")
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_token_protects_non_public_routes_when_configured() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: Some("secret-token".to_owned()),
        });

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(health_response.status(), StatusCode::OK);

        let unauthorized_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);

        let authorized_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .header("authorization", "Bearer secret-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(authorized_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn add_subscription_then_list_feeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let feed_server = spawn_webpage_server(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Local Example Feed</title>
    <link>https://example.com</link>
    <description>Local example feed</description>
    <item>
      <title>Hello</title>
      <link>https://example.com/post</link>
      <guid>item-1</guid>
      <description>Body</description>
    </item>
  </channel>
</rss>"#,
        )
        .await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"feed_url": feed_server.as_str()}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(add_response.status(), StatusCode::OK);
        let add_body = add_response.into_body().collect().await.expect("body bytes").to_bytes();
        let add_json: serde_json::Value = serde_json::from_slice(&add_body).expect("json");
        let feed_id = add_json["feed_id"].as_str().expect("feed id").to_owned();

        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        let feed = storage.get_feed(&feed_id).expect("load feed");
        assert_eq!(feed.title.as_deref(), Some("Local Example Feed"));
        let items = storage.list_items_for_feed(&feed_id, 10, None).expect("list items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Hello");
    }

    #[tokio::test]
    async fn add_subscription_rejects_non_feed_page() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let page_server = spawn_webpage_server("<!doctype html><title>Not a feed</title>").await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"feed_url": page_server.as_str()}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        assert!(storage.list_feeds().expect("list feeds").is_empty());
    }

    #[tokio::test]
    async fn due_feeds_endpoint_returns_ready_feeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage
            .set_feed_next_scheduled_fetch_at(&feed_id, Some("2000-01-01T00:00:00Z"))
            .expect("make feed due");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds/due?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload.as_array().map(|rows| rows.len()), Some(1));
        assert_eq!(payload[0]["id"], feed_id);
        assert_eq!(payload[0]["health_state"], "healthy");
    }

    #[tokio::test]
    async fn item_counts_endpoint_returns_scope_totals() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage
            .upsert_items(&[
                sample_item(&feed_id, "item-1"),
                NormalizedItem {
                    id: "item-2".to_owned(),
                    source_feed_id: feed_id.clone(),
                    external_item_id: Some("ext-2".to_owned()),
                    canonical_url: Some(
                        Url::parse("https://example.com/post-2").expect("url parse"),
                    ),
                    title: "Item Two".to_owned(),
                    author: Some("Author".to_owned()),
                    summary: Some("Summary".to_owned()),
                    content_html: Some("<p>content</p>".to_owned()),
                    content_text: Some("content".to_owned()),
                    published_at: None,
                    updated_at: None,
                    raw_hash: "def".to_owned(),
                    dedup_reason: None,
                    duplicate_of_item_id: None,
                },
            ])
            .expect("upsert items");
        storage
            .patch_item_state(
                "item-1",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(true),
                    is_saved_for_later: Some(true),
                    is_archived: Some(false),
                },
            )
            .expect("patch state");
        storage
            .patch_item_state(
                "item-2",
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(false),
                    is_saved_for_later: Some(false),
                    is_archived: Some(true),
                },
            )
            .expect("patch state");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries/counts")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload["all"], 1);
        assert_eq!(payload["unread"], 0);
        assert_eq!(payload["starred"], 1);
        assert_eq!(payload["later"], 1);
        assert_eq!(payload["archive"], 1);
    }

    #[tokio::test]
    async fn create_entry_endpoint_persists_bookmark_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let webpage_url = spawn_webpage_server(
            r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta property="og:title" content="Example Site" />
    <title>Example Site</title>
  </head>
  <body>
    <main>
      <h1>Welcome</h1>
      <p>Captured content from the homepage.</p>
    </main>
  </body>
</html>"#,
        )
        .await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "",
                            "source_url": webpage_url.as_str(),
                            "canonical_url": webpage_url.as_str(),
                            "summary": "Saved for later"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body =
            create_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&create_body).expect("json");
        let entry_id = payload["id"].as_str().expect("entry id").to_owned();
        assert_eq!(payload["kind"], "bookmark");
        assert_eq!(payload["source_kind"], "web");
        assert_eq!(payload["source_url"], webpage_url.as_str());
        assert_eq!(payload["title"], "Example Site");
        assert!(
            payload["content_html"]
                .as_str()
                .expect("content html")
                .contains("Captured content from the homepage")
        );

        let detail_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/entries/{entry_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_body =
            detail_response.into_body().collect().await.expect("body bytes").to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&detail_body).expect("json");
        assert_eq!(detail["id"], entry_id);
        assert_eq!(detail["kind"], "bookmark");
        assert_eq!(detail["source_kind"], "web");
        assert_eq!(detail["summary"], "Saved for later");
        assert_eq!(detail["title"], "Example Site");
        assert!(detail["content_html"].as_str().expect("detail content html").contains("Welcome"));
    }

    #[tokio::test]
    async fn create_entry_endpoint_persists_note_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "title": "Memos",
                            "kind": "note",
                            "source_kind": "manual",
                            "summary": "memos content here",
                            "content_text": "memos content here"
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create_response.status(), StatusCode::OK);
        let create_body =
            create_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&create_body).expect("json");
        let entry_id = payload["id"].as_str().expect("entry id").to_owned();
        assert_eq!(payload["kind"], "note");
        assert_eq!(payload["source_kind"], "manual");
        assert_eq!(payload["title"], "Memos");

        let counts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries/counts")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(counts_response.status(), StatusCode::OK);
        let counts_body =
            counts_response.into_body().collect().await.expect("body bytes").to_bytes();
        let counts: serde_json::Value = serde_json::from_slice(&counts_body).expect("json");
        assert_eq!(counts["notes"], 1);
        assert_eq!(counts["all"], 1);

        let note_list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries?kind=note")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(note_list_response.status(), StatusCode::OK);
        let note_list_body =
            note_list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let notes: serde_json::Value = serde_json::from_slice(&note_list_body).expect("json");
        assert_eq!(notes.as_array().expect("notes array").len(), 1);
        assert_eq!(notes[0]["kind"], "note");

        let bookmark_list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/entries?kind=bookmark")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(bookmark_list_response.status(), StatusCode::OK);
        let bookmark_list_body =
            bookmark_list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let bookmarks: serde_json::Value =
            serde_json::from_slice(&bookmark_list_body).expect("json");
        assert!(bookmarks.as_array().expect("bookmarks array").is_empty());

        let detail_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/entries/{entry_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_body =
            detail_response.into_body().collect().await.expect("body bytes").to_bytes();
        let detail: serde_json::Value = serde_json::from_slice(&detail_body).expect("json");
        assert_eq!(detail["kind"], "note");
        assert_eq!(detail["source_kind"], "manual");
        assert_eq!(detail["summary"], "memos content here");
    }

    #[tokio::test]
    async fn delete_feed_endpoint_removes_subscription() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let feed_server = spawn_webpage_server(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Delete Me</title>
    <link>https://example.com</link>
    <description>Local example feed</description>
  </channel>
</rss>"#,
        )
        .await;
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscriptions")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"feed_url": feed_server.as_str()}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(add_response.status(), StatusCode::OK);

        let add_body = add_response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&add_body).expect("json");
        let feed_id =
            payload.get("feed_id").and_then(|value| value.as_str()).expect("feed_id").to_owned();

        let delete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri(format!("/api/v1/feeds/{feed_id}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let list_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(list_response.status(), StatusCode::OK);
        let body = list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("utf8 body");
        assert_eq!(text, "[]");
    }

    #[tokio::test]
    async fn notification_settings_and_pending_queue_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let global_settings = GlobalNotificationSettings {
            background_refresh_enabled: false,
            background_refresh_interval_minutes: 30,
            digest_policy: DigestPolicy { enabled: true, interval_minutes: 90, max_items: 12 },
            default_feed_settings: NotificationSettings {
                enabled: true,
                mode: NotificationMode::Digest,
                digest_policy: DigestPolicy { enabled: true, interval_minutes: 60, max_items: 20 },
                quiet_hours: QuietHours {
                    enabled: true,
                    start_minute: 22 * 60,
                    end_minute: 7 * 60,
                },
                minimum_interval_minutes: 45,
                high_priority: false,
                keyword_include: vec!["rust".to_owned()],
                keyword_exclude: vec!["ads".to_owned()],
            },
        };

        let put_global = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/v1/notifications/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(json!(global_settings).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(put_global.status(), StatusCode::OK);

        let get_global = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/settings")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(get_global.status(), StatusCode::OK);
        let body = get_global.into_body().collect().await.expect("body bytes").to_bytes();
        let loaded: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(loaded["background_refresh_enabled"], false);
        assert_eq!(loaded["default_feed_settings"]["mode"], "digest");

        let feed_settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Immediate,
            digest_policy: DigestPolicy { enabled: false, interval_minutes: 60, max_items: 20 },
            quiet_hours: QuietHours { enabled: false, start_minute: 22 * 60, end_minute: 7 * 60 },
            minimum_interval_minutes: 20,
            high_priority: true,
            keyword_include: vec!["rust".to_owned()],
            keyword_exclude: vec![],
        };
        let put_feed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri(format!("/api/v1/feeds/{feed_id}/notifications"))
                    .header("content-type", "application/json")
                    .body(Body::from(json!(feed_settings).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(put_feed.status(), StatusCode::OK);

        let get_feed = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/v1/feeds/{feed_id}/notifications"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(get_feed.status(), StatusCode::OK);

        storage
            .insert_notification_event(
                &NotificationEvent {
                    id: "event-1".to_owned(),
                    feed_id: Some(feed_id.clone()),
                    entry_id: Some("entry-1".to_owned()),
                    canonical_key: "canonical-1".to_owned(),
                    content_fingerprint: "fingerprint-1".to_owned(),
                    title: "Test".to_owned(),
                    body: "Body".to_owned(),
                    mode: NotificationMode::Immediate,
                    delivery_state: NotificationDeliveryState::Pending,
                    reason: "new_item".to_owned(),
                    digest_id: None,
                    created_at: Utc::now(),
                    ready_at: Some(Utc::now()),
                    delivered_at: None,
                    suppressed_at: None,
                },
                Some("{\"source\":\"test\"}"),
            )
            .expect("seed event");

        let pending = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/pending?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(pending.status(), StatusCode::OK);
        let pending_body = pending.into_body().collect().await.expect("body bytes").to_bytes();
        let pending_json: serde_json::Value = serde_json::from_slice(&pending_body).expect("json");
        assert_eq!(pending_json.as_array().map(|items| items.len()), Some(1));

        let ack = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/notifications/pending/ack")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"event_ids":["event-1"]}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ack.status(), StatusCode::OK);

        let pending_after = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/notifications/pending?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = pending_after.into_body().collect().await.expect("body bytes").to_bytes();
        let pending_after_json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(pending_after_json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn patch_item_state_emits_sync_event_then_ack_clears_queue() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        storage.upsert_items(&[sample_item(&feed_id, "item-1")]).expect("upsert items");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let patch_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PATCH)
                    .uri("/api/v1/items/item-1/state")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"is_read": true}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(patch_response.status(), StatusCode::OK);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/sync/events?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = list_response.into_body().collect().await.expect("body bytes").to_bytes();
        let events: serde_json::Value = serde_json::from_slice(&list_body).expect("events json");
        let item_state_event = events
            .as_array()
            .and_then(|items| {
                items.iter().find(|event| {
                    event.get("entity_type").and_then(|value| value.as_str()) == Some("item_state")
                        && event.get("entity_id").and_then(|value| value.as_str()) == Some("item-1")
                        && event.get("event_type").and_then(|value| value.as_str())
                            == Some("updated")
                })
            })
            .expect("item state event");
        let event_id =
            item_state_event.get("id").and_then(|id| id.as_str()).expect("event id").to_owned();

        let ack_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/sync/events/ack")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"event_ids":[event_id]}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ack_response.status(), StatusCode::OK);
        let ack_body = ack_response.into_body().collect().await.expect("body bytes").to_bytes();
        let ack_payload: serde_json::Value = serde_json::from_slice(&ack_body).expect("ack json");
        assert_eq!(ack_payload["acknowledged"], 1);

        let list_after_ack = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/sync/events?limit=10")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list_after_ack.status(), StatusCode::OK);
        let list_after_ack_body =
            list_after_ack.into_body().collect().await.expect("body bytes").to_bytes();
        let events_after_ack: serde_json::Value =
            serde_json::from_slice(&list_after_ack_body).expect("events json");
        assert!(
            events_after_ack
                .as_array()
                .expect("events array")
                .iter()
                .all(|event| event.get("entity_type").and_then(|value| value.as_str())
                    != Some("item_state")),
            "item_state event should have been acknowledged"
        );
    }

    #[tokio::test]
    async fn import_opml_endpoint_creates_feed_and_group() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let opml_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Subscriptions</title></head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Example" title="Example" type="rss" xmlUrl="https://example.com/feed.xml" htmlUrl="https://example.com" />
    </outline>
  </body>
</opml>"#;

        let import_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/opml/import")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "opml_xml": opml_xml }).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(import_response.status(), StatusCode::OK);
        let import_body = import_response.into_body().collect().await.expect("body").to_bytes();
        let import_payload: serde_json::Value = serde_json::from_slice(&import_body).expect("json");
        assert_eq!(import_payload["parsed_feed_count"], 1);
        assert_eq!(import_payload["unique_feed_count"], 1);
        assert_eq!(import_payload["grouped_feed_count"], 1);

        let feeds_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/feeds")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(feeds_response.status(), StatusCode::OK);
        let feeds_body = feeds_response.into_body().collect().await.expect("body").to_bytes();
        let feeds: serde_json::Value = serde_json::from_slice(&feeds_body).expect("json");
        assert_eq!(feeds.as_array().map(|rows| rows.len()), Some(1));
        assert_eq!(feeds[0]["feed_url"], "https://example.com/feed.xml");

        let groups_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/groups")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(groups_response.status(), StatusCode::OK);
        let groups_body = groups_response.into_body().collect().await.expect("body").to_bytes();
        let groups: serde_json::Value = serde_json::from_slice(&groups_body).expect("json");
        assert!(groups.as_array().is_some_and(|rows| rows.iter().any(|row| row["name"] == "Tech")));
    }

    #[tokio::test]
    async fn export_opml_endpoint_returns_subscription_xml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");

        let mut storage = Storage::open(db.to_string_lossy().as_ref()).expect("open storage");
        storage.migrate().expect("migrate");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("url parse"),
                site_url: Some(Url::parse("https://example.com").expect("url parse")),
                title: Some("Example".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("upsert feed");
        let group = storage.create_group("Tech").expect("create group");
        storage.set_feed_group(&feed_id, Some(&group.id)).expect("assign group");

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 3,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/v1/opml/export")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        let xml = payload["opml_xml"].as_str().expect("opml xml");
        assert_eq!(payload["feed_count"], 1);
        assert!(xml.contains("xmlUrl=\"https://example.com/feed.xml\""));
        assert!(xml.contains("title=\"Tech\"") || xml.contains("text=\"Tech\""));
    }

    #[test]
    fn fallback_candidates_cover_domain_blog_paths() {
        let site_url = url::Url::parse("https://example.com").expect("site url");
        let candidates = fallback_candidate_urls(&site_url);
        let urls: Vec<String> = candidates.into_iter().map(|url| url.to_string()).collect();

        assert!(urls.iter().any(|url| url == "https://example.com/blog/atom.xml"));
        assert!(urls.iter().any(|url| url == "https://example.com/posts.atom"));
    }

    #[test]
    fn runtime_config_accepts_cli_overrides() {
        let config = resolve_runtime_config(vec![
            "--port".to_owned(),
            "4321".to_owned(),
            "--db-path".to_owned(),
            "/tmp/infomatrix-cli-test.db".to_owned(),
        ])
        .expect("resolve config");

        assert_eq!(config.bind_addr.port(), 4321);
        assert_eq!(config.db_path, "/tmp/infomatrix-cli-test.db");
    }

    #[test]
    fn runtime_config_rejects_remote_bind_without_explicit_allow() {
        let result = resolve_runtime_config_with_env(
            vec!["--bind-addr".to_owned(), "0.0.0.0:3199".to_owned()],
            |_| None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn runtime_config_rejects_remote_bind_without_token() {
        let result = resolve_runtime_config_with_env(
            vec!["--bind-addr".to_owned(), "0.0.0.0:3199".to_owned()],
            |name| match name {
                "INFOMATRIX_ALLOW_REMOTE_BIND" => Some("1".to_owned()),
                _ => None,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn runtime_config_accepts_remote_bind_with_allow_and_token() {
        let config = resolve_runtime_config_with_env(
            vec!["--bind-addr".to_owned(), "0.0.0.0:3199".to_owned()],
            |name| match name {
                "INFOMATRIX_ALLOW_REMOTE_BIND" => Some("1".to_owned()),
                "INFOMATRIX_API_TOKEN" => Some("secret".to_owned()),
                _ => None,
            },
        )
        .expect("resolve config");

        assert!(!config.bind_addr.ip().is_loopback());
        assert_eq!(config.api_token.as_deref(), Some("secret"));
    }

    #[test]
    fn markdown_content_is_rendered_to_html() {
        let html = prepare_display_html(
            None,
            Some("```rust\nfn main() {}\n```\n\n    let answer = 42\n"),
            None,
        )
        .expect("rendered html");
        assert!(html.contains("<pre><code"));
        assert!(html.contains("let answer = 42"));
    }

    #[test]
    fn summary_html_can_be_promoted_to_display_html() {
        let html = prepare_display_html(None, None, Some("<p>Hello <strong>RSS</strong></p>"))
            .expect("summary html");
        assert!(html.contains("<strong>RSS</strong>"));
    }

    #[test]
    fn extracted_full_content_keeps_code_blocks_once() {
        let html = r#"
            <html>
              <body>
                <main>
                  <article class="article-prose">
                    <p>Hello world</p>
                    <pre><code>let answer = 42</code></pre>
                    <p>Goodbye</p>
                  </article>
                </main>
              </body>
            </html>
        "#;

        let extracted = extract_full_content(html).expect("full content");
        assert_eq!(extracted.source, "web_extract");
        assert_eq!(extracted.content_text.matches("let answer = 42").count(), 1);
        assert!(extracted.content_text.contains("Hello world"));
        assert!(extracted.content_html.as_deref().unwrap_or_default().contains("<pre><code>"));
    }

    #[test]
    fn preview_strips_html_tags() {
        let preview = clean_preview_text("<p>Hello <strong>world</strong></p>").expect("preview");
        assert_eq!(preview, "Hello world");
    }

    #[test]
    fn candidate_urls_cover_scheme_and_www_variants() {
        let base = url::Url::parse("https://example.com/blog").expect("url");
        let candidates = candidate_site_urls(&base);
        let values: Vec<String> = candidates.into_iter().map(|value| value.to_string()).collect();
        assert!(values.iter().any(|value| value == "https://example.com/blog"));
        assert!(values.iter().any(|value| value == "http://example.com/blog"));
        assert!(values.iter().any(|value| value == "https://www.example.com/blog"));
    }

    #[test]
    fn scoring_prefers_primary_feed_over_comments_feed() {
        let main_score =
            feed_candidate_score("https://example.com/feed", Some("Main Feed"), "autodiscovery", 1);
        let comments_score = feed_candidate_score(
            "https://example.com/comments/feed",
            Some("Comments Feed"),
            "autodiscovery",
            1,
        );
        assert!(main_score > comments_score);
    }

    #[test]
    fn confidence_from_score_is_clamped() {
        assert!((0.15..=0.99).contains(&confidence_from_score(-200)));
        assert!((0.15..=0.99).contains(&confidence_from_score(500)));
    }

    async fn spawn_site_with_feed_server(html: &'static str, feed_xml: &'static str) -> Url {
        let app = axum::Router::new()
            .route("/", get(move || async move { Html(html) }))
            .route("/feed.xml", get(move || async move { Html(feed_xml) }));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind test site server");
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test site");
        });
        Url::parse(&format!("http://{addr}/")).expect("test site url")
    }

    #[tokio::test]
    async fn subscribe_input_falls_back_to_site_discovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join("infomatrix.db");
        let site_url = spawn_site_with_feed_server(
            r#"<html><head><link rel="alternate" type="application/rss+xml" href="/feed.xml" title="Feed"></head><body>Site</body></html>"#,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Discovered Feed</title>
    <link>https://example.com</link>
  </channel>
</rss>"#,
        )
        .await;

        let app = app_router(AppContext {
            db_path: db.to_string_lossy().to_string(),
            user_agent: "test-agent".to_owned(),
            timeout_secs: 5,
            migrate_on_open: true,
            api_token: None,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/subscribe")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "input_url": site_url.to_string() }).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.expect("body bytes").to_bytes();
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload["subscription_source"], "discovery");
        assert_eq!(
            payload["resolved_feed_url"].as_str().expect("resolved_feed_url"),
            format!("{}feed.xml", site_url)
        );
    }
}
