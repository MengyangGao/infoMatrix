use url::Url;

use crate::SharedApiError;

pub fn normalize_input_url(input: &str) -> Result<String, SharedApiError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SharedApiError::BadRequest("input url is empty".to_owned()));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_owned());
    }
    Ok(format!("https://{trimmed}"))
}

pub fn enforce_web_url(url: &Url, label: &str) -> Result<(), SharedApiError> {
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(SharedApiError::BadRequest(format!(
            "{label} must use http or https scheme, got: {other}"
        ))),
    }
}

pub fn derive_site_url_from_feed(feed_url: &Url) -> Option<Url> {
    let host = feed_url.host_str()?;
    Url::parse(&format!("{}://{host}", feed_url.scheme())).ok()
}

pub fn feed_icon_url(site_url: Option<&Url>, feed_url: &Url) -> Option<String> {
    let host = site_url
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .or_else(|| feed_url.host_str().map(ToOwned::to_owned))?;
    Some(format!("https://{host}/favicon.ico"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_input_url_adds_https_and_preserves_existing_scheme() {
        assert_eq!(normalize_input_url("example.com").unwrap(), "https://example.com");
        assert_eq!(
            normalize_input_url("  https://example.com/path  ").unwrap(),
            "https://example.com/path"
        );
        assert_eq!(normalize_input_url("http://example.com").unwrap(), "http://example.com");
    }

    #[test]
    fn normalize_input_url_rejects_empty() {
        assert!(normalize_input_url("   ").is_err());
    }

    #[test]
    fn enforce_web_url_rejects_non_web_schemes() {
        let web = Url::parse("https://example.com").unwrap();
        assert!(enforce_web_url(&web, "url").is_ok());

        let ftp = Url::parse("ftp://example.com").unwrap();
        assert!(enforce_web_url(&ftp, "url").is_err());
    }

    #[test]
    fn derive_site_url_from_feed_strips_path() {
        let feed = Url::parse("https://example.com/feed.xml").unwrap();
        assert_eq!(derive_site_url_from_feed(&feed).unwrap().as_str(), "https://example.com/");
    }

    #[test]
    fn feed_icon_url_prefers_site_host_then_feed_host() {
        let feed = Url::parse("https://feed.example.com/rss").unwrap();
        let site = Url::parse("https://example.com").unwrap();
        assert_eq!(feed_icon_url(Some(&site), &feed).unwrap(), "https://example.com/favicon.ico");

        assert_eq!(feed_icon_url(None, &feed).unwrap(), "https://feed.example.com/favicon.ico");
    }
}
