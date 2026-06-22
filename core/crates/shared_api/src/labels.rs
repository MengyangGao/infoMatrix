use models::{EntryKind, EntrySourceKind, FeedType};
use storage::ItemListFilter;

pub fn entry_kind_label(value: EntryKind) -> &'static str {
    match value {
        EntryKind::Article => "article",
        EntryKind::Bookmark => "bookmark",
        EntryKind::Note => "note",
        EntryKind::Quote => "quote",
    }
}

pub fn entry_source_kind_label(value: EntrySourceKind) -> &'static str {
    match value {
        EntrySourceKind::Feed => "feed",
        EntrySourceKind::Web => "web",
        EntrySourceKind::Manual => "manual",
        EntrySourceKind::Import => "import",
        EntrySourceKind::Sync => "sync",
    }
}

pub fn feed_type_label(value: FeedType) -> &'static str {
    match value {
        FeedType::Rss => "rss",
        FeedType::Atom => "atom",
        FeedType::JsonFeed => "jsonfeed",
        FeedType::Unknown => "unknown",
    }
}

pub fn parse_item_filter(value: Option<&str>) -> ItemListFilter {
    match value.unwrap_or("all").trim().to_ascii_lowercase().as_str() {
        "unread" => ItemListFilter::Unread,
        "starred" => ItemListFilter::Starred,
        "later" => ItemListFilter::Later,
        "archive" | "archived" => ItemListFilter::Archive,
        _ => ItemListFilter::All,
    }
}

pub fn parse_entry_kind_label(value: &str) -> EntryKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "bookmark" => EntryKind::Bookmark,
        "note" => EntryKind::Note,
        "quote" => EntryKind::Quote,
        _ => EntryKind::Article,
    }
}

pub fn parse_entry_source_kind_label(value: &str) -> EntrySourceKind {
    match value.trim().to_ascii_lowercase().as_str() {
        "web" => EntrySourceKind::Web,
        "manual" => EntrySourceKind::Manual,
        "import" => EntrySourceKind::Import,
        "sync" => EntrySourceKind::Sync,
        _ => EntrySourceKind::Feed,
    }
}

pub fn score_from_confidence(confidence: f32) -> i32 {
    (confidence * 120.0 - 30.0).round() as i32
}

pub fn confidence_from_score(score: i32) -> f32 {
    ((score + 30) as f32 / 120.0).clamp(0.15, 0.99)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_kind_labels_round_trip() {
        assert_eq!(entry_kind_label(EntryKind::Article), "article");
        assert_eq!(entry_kind_label(EntryKind::Bookmark), "bookmark");
        assert_eq!(entry_kind_label(EntryKind::Note), "note");
        assert_eq!(entry_kind_label(EntryKind::Quote), "quote");
    }

    #[test]
    fn parse_entry_kind_label_defaults_to_article() {
        assert_eq!(parse_entry_kind_label("bookmark"), EntryKind::Bookmark);
        assert_eq!(parse_entry_kind_label("  NOTE \n"), EntryKind::Note);
        assert_eq!(parse_entry_kind_label("unknown"), EntryKind::Article);
    }

    #[test]
    fn feed_type_labels_match_expected_values() {
        assert_eq!(feed_type_label(FeedType::Rss), "rss");
        assert_eq!(feed_type_label(FeedType::Atom), "atom");
        assert_eq!(feed_type_label(FeedType::JsonFeed), "jsonfeed");
        assert_eq!(feed_type_label(FeedType::Unknown), "unknown");
    }

    #[test]
    fn parse_item_filter_handles_variants() {
        assert!(matches!(parse_item_filter(Some("unread")), ItemListFilter::Unread));
        assert!(matches!(parse_item_filter(Some("STARRED")), ItemListFilter::Starred));
        assert!(matches!(parse_item_filter(Some("archive")), ItemListFilter::Archive));
        assert!(matches!(parse_item_filter(Some("archived")), ItemListFilter::Archive));
        assert!(matches!(parse_item_filter(None), ItemListFilter::All));
        assert!(matches!(parse_item_filter(Some("nonsense")), ItemListFilter::All));
    }

    #[test]
    fn confidence_score_conversions_are_inverses_near_middle() {
        let original = 0.75_f32;
        let score = score_from_confidence(original);
        let recovered = confidence_from_score(score);
        assert!(
            (recovered - original).abs() < 0.01,
            "recovered {} != original {}",
            recovered,
            original
        );
    }

    #[test]
    fn confidence_from_score_is_clamped() {
        assert_eq!(confidence_from_score(-200), 0.15);
        assert_eq!(confidence_from_score(500), 0.99);
    }
}
