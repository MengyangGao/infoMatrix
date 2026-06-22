use reqwest::Client;
use scraper::{Html, Selector};
use url::Url;

use crate::SharedApiError;

#[derive(Debug, Clone)]
pub struct WebpageCapture {
    pub final_url: Url,
    pub title: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExtractedContent {
    pub content_html: Option<String>,
    pub content_text: String,
    pub source: &'static str,
}

pub async fn capture_webpage_snapshot(
    client: &Client,
    url: &Url,
) -> Result<WebpageCapture, SharedApiError> {
    let response = client.get(url.clone()).send().await?;

    if !response.status().is_success() {
        return Err(SharedApiError::BadRequest(format!(
            "webpage capture failed with HTTP {}",
            response.status().as_u16()
        )));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase());
    if let Some(content_type) = content_type.as_deref() {
        let looks_like_html = content_type.contains("html")
            || content_type.contains("xml")
            || content_type.contains("xhtml");
        if !looks_like_html {
            return Err(SharedApiError::BadRequest(format!(
                "webpage capture only supports html content, got: {content_type}"
            )));
        }
    }

    let html = response.text().await?;
    let document = Html::parse_document(&html);
    let title = extract_webpage_title(&document)
        .or_else(|| webpage_fallback_title(&final_url))
        .filter(|value| !value.trim().is_empty());
    let content_html = extract_webpage_body_html(&document);
    let content_text = extract_webpage_body_text(&document);

    Ok(WebpageCapture { final_url, title, content_html, content_text })
}

pub fn extract_full_content(html: &str) -> Option<ExtractedContent> {
    let document = Html::parse_document(html);

    for selector in [
        "article",
        "main",
        ".article-prose",
        ".markdown-body",
        ".post-content",
        ".entry-content",
        ".post",
        "body",
    ] {
        if let Some(extracted) = extract_from_selector(&document, selector) {
            return Some(extracted);
        }
    }

    None
}

pub fn extract_from_selector(document: &Html, selector: &str) -> Option<ExtractedContent> {
    let selector = Selector::parse(selector).ok()?;
    let block_selector =
        Selector::parse("p,pre,li,h1,h2,h3,h4,h5,h6,blockquote,table,thead,tbody,tr,th,td").ok()?;

    for node in document.select(&selector) {
        let mut blocks = Vec::new();
        let mut saw_preformatted = false;
        for block in node.select(&block_selector) {
            let tag = block.value().name();
            let raw = block.text().collect::<String>();
            let text = if tag == "pre" {
                saw_preformatted = true;
                raw.trim().to_owned()
            } else {
                normalize_whitespace(&raw)
            };
            if text.len() >= 8 {
                blocks.push(text);
            }
        }

        let content_text = if blocks.is_empty() {
            normalize_whitespace(&node.text().collect::<String>())
        } else {
            blocks.join("\n\n")
        };

        let minimum_length = if saw_preformatted { 1 } else { 240 };
        if content_text.len() >= minimum_length {
            let content_html = Some(node.html());
            return Some(ExtractedContent { content_html, content_text, source: "web_extract" });
        }
    }

    None
}

pub fn sanitize_html_fragment(html: &str) -> String {
    ammonia::Builder::new().clean(html).to_string()
}

pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn webpage_fallback_title(url: &Url) -> Option<String> {
    let host = url.host_str()?.trim();
    if host.is_empty() {
        return None;
    }

    let path = url.path().trim_matches('/');
    if path.is_empty() {
        return Some(host.to_owned());
    }

    let last_segment = path
        .split('/')
        .rev()
        .find(|segment| !segment.trim().is_empty())
        .map(|segment| segment.trim().to_owned())?;
    Some(format!("{host} · {last_segment}"))
}

pub fn extract_webpage_title(document: &Html) -> Option<String> {
    for selector in
        ["meta[property='og:title']", "meta[name='twitter:title']", "meta[name='title']"]
    {
        if let Some(value) = extract_meta_content(document, selector) {
            return Some(value);
        }
    }

    let selector = Selector::parse("title").ok()?;
    if let Some(value) = document
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Some(value);
    }

    let selector = Selector::parse("h1").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn extract_meta_content(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn extract_webpage_body_html(document: &Html) -> Option<String> {
    let selector = Selector::parse("body").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| node.inner_html())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub fn extract_webpage_body_text(document: &Html) -> Option<String> {
    let selector = Selector::parse("body").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| normalize_whitespace(&node.text().collect::<String>()))
        .filter(|value| !value.is_empty())
}
