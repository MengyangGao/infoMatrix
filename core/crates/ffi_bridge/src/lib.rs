//! Flutter-facing Rust FFI bridge for InfoMatrix core capabilities.

use std::sync::LazyLock;

pub mod content;
pub mod discovery;
pub mod entries;
pub mod envelope;
pub mod ffi_wrappers;
pub mod labels;
pub mod notifications;
pub mod opml;
pub mod refresh;
pub mod storage;
pub mod subscriptions;
pub mod sync;

pub use envelope::infomatrix_core_free_string;
pub use ffi_wrappers::*;

static TOKIO_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
});

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString, c_char};

    use models::{FeedType, NewFeed};
    use url::Url;

    use crate::storage::open_storage;
    use crate::{ffi_wrappers::*, infomatrix_core_free_string};

    fn decode_envelope(output: *mut c_char) -> serde_json::Value {
        let json = unsafe { CStr::from_ptr(output) }.to_str().expect("utf8").to_owned();
        unsafe { infomatrix_core_free_string(output) };
        serde_json::from_str(&json).expect("json parse")
    }

    #[test]
    fn health_json_contains_ok() {
        let output = infomatrix_core_health_json();
        let value = decode_envelope(output);
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["status"], "ok");
    }

    #[test]
    fn can_add_and_list_subscription() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
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
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": format!("http://127.0.0.1:{port}/feed.xml"),
            "title": "Example Feed"
        });

        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        assert_eq!(add_envelope["ok"], true);
        let feed_id = add_envelope["data"]["feed_id"].as_str().expect("feed id").to_owned();

        let list_entries_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10
        });
        let list_entries_input = CString::new(list_entries_payload.to_string()).expect("cstring");
        let list_entries_output =
            unsafe { infomatrix_core_list_entries_json(list_entries_input.as_ptr()) };
        let list_entries_envelope = decode_envelope(list_entries_output);
        assert_eq!(list_entries_envelope["ok"], true);
        let entries = list_entries_envelope["data"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["title"], "Hello");

        let search_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10,
            "q": "missing"
        });
        let search_input = CString::new(search_payload.to_string()).expect("cstring");
        let search_output = unsafe { infomatrix_core_list_items_json(search_input.as_ptr()) };
        let search_envelope = decode_envelope(search_output);
        assert_eq!(search_envelope["ok"], true);
        let search_entries = search_envelope["data"].as_array().expect("search entries array");
        assert!(search_entries.is_empty());

        let list_payload = serde_json::json!({ "db_path": temp.path().to_string_lossy() });
        let list_input = CString::new(list_payload.to_string()).expect("cstring");
        let list_output = unsafe { infomatrix_core_list_feeds_json(list_input.as_ptr()) };
        let list_envelope = decode_envelope(list_output);

        assert_eq!(list_envelope["ok"], true);
        let feeds = list_envelope["data"].as_array().expect("array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["title"], "Local Example Feed");
        assert_eq!(feeds[0]["icon_url"], "https://example.com/favicon.ico");

        let _ = server_handle.join();
    }

    #[test]
    fn add_subscription_rejects_non_feed_response() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = "<!doctype html><title>Not a feed</title>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": format!("http://127.0.0.1:{port}/not-feed.html"),
            "title": "Not a Feed"
        });
        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        assert_eq!(add_envelope["ok"], false, "add envelope: {add_envelope}");

        let list_payload = serde_json::json!({ "db_path": temp.path().to_string_lossy() });
        let list_input = CString::new(list_payload.to_string()).expect("cstring");
        let list_output = unsafe { infomatrix_core_list_feeds_json(list_input.as_ptr()) };
        let list_envelope = decode_envelope(list_output);
        assert_eq!(list_envelope["ok"], true);
        let feeds = list_envelope["data"].as_array().expect("feeds array");
        assert!(feeds.is_empty());

        let _ = server_handle.join();
    }

    #[test]
    fn subscribe_direct_feed_returns_resolved_feed_url() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
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
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");
        let subscribe_payload = serde_json::json!({
            "db_path": db_path,
            "input_url": feed_url
        });
        let subscribe_input = CString::new(subscribe_payload.to_string()).expect("cstring");
        let subscribe_output =
            unsafe { infomatrix_core_subscribe_input_json(subscribe_input.as_ptr()) };
        let subscribe_envelope = decode_envelope(subscribe_output);

        assert_eq!(subscribe_envelope["ok"], true, "subscribe envelope: {subscribe_envelope}");
        assert_eq!(subscribe_envelope["data"]["resolved_feed_url"], feed_url);
        assert_eq!(subscribe_envelope["data"]["subscription_source"], "direct_feed");
        assert!(
            subscribe_envelope["data"]["feed_id"].as_str().is_some(),
            "feed id missing: {subscribe_envelope}"
        );

        let feed_id = subscribe_envelope["data"]["feed_id"].as_str().expect("feed id");
        let list_entries_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10
        });
        let list_entries_input = CString::new(list_entries_payload.to_string()).expect("cstring");
        let list_entries_output =
            unsafe { infomatrix_core_list_entries_json(list_entries_input.as_ptr()) };
        let list_entries_envelope = decode_envelope(list_entries_output);
        assert_eq!(list_entries_envelope["ok"], true);
        let entries = list_entries_envelope["data"].as_array().expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["title"], "Hello");

        let list_feeds_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_feeds_input = CString::new(list_feeds_payload.to_string()).expect("cstring");
        let list_feeds_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_feeds_envelope = decode_envelope(list_feeds_output);
        let feeds = list_feeds_envelope["data"].as_array().expect("feeds array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["icon_url"], "https://example.com/favicon.ico");

        let _ = server_handle.join();
    }

    #[test]
    fn refresh_feed_persists_new_items() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let initial_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Refresh Fixture</title>
    <link>https://example.com</link>
    <description>Refresh fixture feed</description>
    <item>
      <title>Initial Item</title>
      <link>https://example.com/initial</link>
      <guid>item-1</guid>
      <description>Initial body</description>
    </item>
  </channel>
</rss>"#;
        let refreshed_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Refresh Fixture</title>
    <link>https://example.com</link>
    <description>Refresh fixture feed</description>
    <item>
      <title>Second Item</title>
      <link>https://example.com/second</link>
      <guid>item-2</guid>
      <description>Second body</description>
    </item>
    <item>
      <title>Initial Item</title>
      <link>https://example.com/initial</link>
      <guid>item-1</guid>
      <description>Initial body</description>
    </item>
  </channel>
</rss>"#;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            for body in [initial_body, refreshed_body] {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buffer = [0u8; 2048];
                    let _ = std::io::Read::read(&mut stream, &mut buffer);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                    let _ = std::io::Write::flush(&mut stream);
                }
            }
        });

        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");
        let subscribe_payload = serde_json::json!({
            "db_path": db_path,
            "input_url": feed_url
        });
        let subscribe_input = CString::new(subscribe_payload.to_string()).expect("cstring");
        let subscribe_output =
            unsafe { infomatrix_core_subscribe_input_json(subscribe_input.as_ptr()) };
        let subscribe_envelope = decode_envelope(subscribe_output);
        assert_eq!(subscribe_envelope["ok"], true, "subscribe envelope: {subscribe_envelope}");
        let feed_id = subscribe_envelope["data"]["feed_id"].as_str().expect("feed id").to_owned();

        let refresh_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id
        });
        let refresh_input = CString::new(refresh_payload.to_string()).expect("cstring");
        let refresh_output = unsafe { infomatrix_core_refresh_feed_json(refresh_input.as_ptr()) };
        let refresh_envelope = decode_envelope(refresh_output);
        assert_eq!(refresh_envelope["ok"], true, "refresh envelope: {refresh_envelope}");
        assert_eq!(refresh_envelope["data"]["status"], "updated");
        assert_eq!(refresh_envelope["data"]["item_count"], 2);

        let list_entries_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "limit": 10
        });
        let list_entries_input = CString::new(list_entries_payload.to_string()).expect("cstring");
        let list_entries_output =
            unsafe { infomatrix_core_list_entries_json(list_entries_input.as_ptr()) };
        let list_entries_envelope = decode_envelope(list_entries_output);
        assert_eq!(list_entries_envelope["ok"], true);
        let entries = list_entries_envelope["data"].as_array().expect("entries array");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry["title"] == "Second Item"));

        let _ = server_handle.join();
    }

    #[test]
    fn create_entry_captures_webpage_snapshot() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 2048];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<!doctype html>
<html>
  <head><title>Example Page</title></head>
  <body>
    <main>
      <article><p>Story body</p></article>
    </main>
  </body>
</html>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let source_url = format!("http://127.0.0.1:{port}/story.html");
        let create_payload = serde_json::json!({
            "db_path": db_path,
            "title": "",
            "kind": "bookmark",
            "source_kind": "web",
            "source_url": source_url
        });
        let create_input = CString::new(create_payload.to_string()).expect("cstring");
        let create_output = unsafe { infomatrix_core_create_entry_json(create_input.as_ptr()) };
        let create_envelope = decode_envelope(create_output);
        assert_eq!(create_envelope["ok"], true, "create envelope: {create_envelope}");
        assert_eq!(create_envelope["data"]["title"], "Example Page");
        assert!(
            create_envelope["data"]["content_text"]
                .as_str()
                .expect("content_text")
                .contains("Story body")
        );

        let _ = server_handle.join();
    }

    #[test]
    fn can_manage_groups_and_feed_metadata() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let storage = open_storage(&Some(db_path.clone())).expect("open storage");
        let feed_id = storage
            .upsert_feed(&NewFeed {
                feed_url: Url::parse("https://example.com/feed.xml").expect("feed url"),
                site_url: Some(Url::parse("https://example.com").expect("site url")),
                title: Some("Example Feed".to_owned()),
                feed_type: FeedType::Rss,
            })
            .expect("seed feed");

        let create_group_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "name": "Tech"
        });
        let create_group_input = CString::new(create_group_payload.to_string()).expect("cstring");
        let create_group_output =
            unsafe { infomatrix_core_create_group_json(create_group_input.as_ptr()) };
        let create_group_envelope = decode_envelope(create_group_output);
        assert_eq!(create_group_envelope["ok"], true);
        assert_eq!(create_group_envelope["data"]["name"], "Tech");

        let list_groups_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_groups_input = CString::new(list_groups_payload.to_string()).expect("cstring");
        let list_groups_output =
            unsafe { infomatrix_core_list_groups_json(list_groups_input.as_ptr()) };
        let list_groups_envelope = decode_envelope(list_groups_output);
        let groups = list_groups_envelope["data"].as_array().expect("groups array");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["name"], "Tech");

        let update_feed_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "title": "Renamed Feed"
        });
        let update_feed_input = CString::new(update_feed_payload.to_string()).expect("cstring");
        let update_feed_output =
            unsafe { infomatrix_core_update_feed_json(update_feed_input.as_ptr()) };
        let update_feed_envelope = decode_envelope(update_feed_output);
        assert_eq!(update_feed_envelope["ok"], true);

        let update_group_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id,
            "group_id": create_group_envelope["data"]["id"].as_str().expect("group id")
        });
        let update_group_input = CString::new(update_group_payload.to_string()).expect("cstring");
        let update_group_output =
            unsafe { infomatrix_core_update_feed_group_json(update_group_input.as_ptr()) };
        let update_group_envelope = decode_envelope(update_group_output);
        assert_eq!(update_group_envelope["ok"], true);

        let list_feeds_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let list_feeds_input = CString::new(list_feeds_payload.to_string()).expect("cstring");
        let list_feeds_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_feeds_envelope = decode_envelope(list_feeds_output);
        let feeds = list_feeds_envelope["data"].as_array().expect("feeds array");
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0]["title"], "Renamed Feed");
        let feed_groups = feeds[0]["groups"].as_array().expect("feed groups");
        assert_eq!(feed_groups.len(), 1);
        assert_eq!(feed_groups[0]["name"], "Tech");

        let delete_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "feed_id": feed_id
        });
        let delete_input = CString::new(delete_payload.to_string()).expect("cstring");
        let delete_output = unsafe { infomatrix_core_delete_feed_json(delete_input.as_ptr()) };
        let delete_envelope = decode_envelope(delete_output);
        assert_eq!(delete_envelope["ok"], true);

        let list_after_delete_output =
            unsafe { infomatrix_core_list_feeds_json(list_feeds_input.as_ptr()) };
        let list_after_delete_envelope = decode_envelope(list_after_delete_output);
        assert!(list_after_delete_envelope["data"].as_array().expect("feeds array").is_empty());
    }

    #[test]
    fn can_import_and_export_opml() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();
        let opml_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>Subscriptions</title></head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Example" title="Example" type="rss" xmlUrl="https://example.com/feed.xml" htmlUrl="https://example.com" />
    </outline>
  </body>
</opml>"#;

        let import_payload = serde_json::json!({
            "db_path": db_path,
            "opml_xml": opml_xml
        });
        let import_input = CString::new(import_payload.to_string()).expect("cstring");
        let import_output = unsafe { infomatrix_core_import_opml_json(import_input.as_ptr()) };
        let import_envelope = decode_envelope(import_output);
        assert_eq!(import_envelope["ok"], true);
        assert_eq!(import_envelope["data"]["parsed_feed_count"], 1);
        assert_eq!(import_envelope["data"]["unique_feed_count"], 1);
        assert_eq!(import_envelope["data"]["grouped_feed_count"], 1);

        let export_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let export_input = CString::new(export_payload.to_string()).expect("cstring");
        let export_output = unsafe { infomatrix_core_export_opml_json(export_input.as_ptr()) };
        let export_envelope = decode_envelope(export_output);
        assert_eq!(export_envelope["ok"], true);
        assert_eq!(export_envelope["data"]["feed_count"], 1);
        let xml = export_envelope["data"]["opml_xml"].as_str().expect("xml");
        assert!(xml.contains("xmlUrl=\"https://example.com/feed.xml\""));
    }

    #[test]
    fn can_refresh_due_feeds() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let port = listener.local_addr().expect("local addr").port();
        let server_handle = std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buffer = [0u8; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buffer);
                let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com</link>
    <description>Example</description>
    <item>
      <title>Hello</title>
      <link>https://example.com/post</link>
      <guid>item-1</guid>
      <description>Body</description>
    </item>
  </channel>
</rss>"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                let _ = std::io::Write::flush(&mut stream);
            }
        });

        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");

        let add_payload = serde_json::json!({
            "db_path": db_path,
            "feed_url": feed_url,
            "title": "Example Feed"
        });
        let add_input = CString::new(add_payload.to_string()).expect("cstring");
        let add_output = unsafe { infomatrix_core_add_subscription_json(add_input.as_ptr()) };
        let add_envelope = decode_envelope(add_output);
        let feed_id = add_envelope["data"]["feed_id"].as_str().expect("feed id");

        let storage =
            open_storage(&Some(temp.path().to_string_lossy().to_string())).expect("open storage");
        storage
            .set_feed_next_scheduled_fetch_at(feed_id, Some("2000-01-01T00:00:00Z"))
            .expect("make feed due");

        let refresh_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "limit": 10
        });
        let refresh_input = CString::new(refresh_payload.to_string()).expect("cstring");
        let refresh_output = unsafe { infomatrix_core_refresh_due_json(refresh_input.as_ptr()) };
        let refresh_envelope = decode_envelope(refresh_output);
        assert_eq!(refresh_envelope["ok"], true, "refresh envelope: {refresh_envelope}");
        assert_eq!(refresh_envelope["data"]["refreshed_count"], 1);

        let _ = server_handle.join();
    }

    #[test]
    fn can_create_note_and_count_notes() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = temp.path().to_string_lossy().to_string();

        let create_payload = serde_json::json!({
            "db_path": db_path,
            "title": "Quick note",
            "kind": "note",
            "source_kind": "manual",
            "content_text": "Body"
        });
        let create_input = CString::new(create_payload.to_string()).expect("cstring");
        let create_output = unsafe { infomatrix_core_create_entry_json(create_input.as_ptr()) };
        let create_envelope = decode_envelope(create_output);
        assert_eq!(create_envelope["ok"], true);
        assert_eq!(create_envelope["data"]["kind"], "note");
        let item_id = create_envelope["data"]["id"].as_str().expect("id").to_owned();

        let list_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy(),
            "filter": "all",
            "limit": 20,
            "kind": "note"
        });
        let list_input = CString::new(list_payload.to_string()).expect("cstring");
        let list_output = unsafe { infomatrix_core_list_entries_json(list_input.as_ptr()) };
        let list_envelope = decode_envelope(list_output);
        assert_eq!(list_envelope["ok"], true);
        let rows = list_envelope["data"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], item_id);

        let counts_payload = serde_json::json!({
            "db_path": temp.path().to_string_lossy()
        });
        let counts_input = CString::new(counts_payload.to_string()).expect("cstring");
        let counts_output = unsafe { infomatrix_core_item_counts_json(counts_input.as_ptr()) };
        let counts_envelope = decode_envelope(counts_output);
        assert_eq!(counts_envelope["ok"], true);
        assert_eq!(counts_envelope["data"]["notes"], 1);
    }
}
