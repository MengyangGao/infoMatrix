use ammonia::Builder as HtmlSanitizerBuilder;
use pulldown_cmark::{Options as MarkdownOptions, Parser as MarkdownParser, html as markdown_html};
pub use shared_api::webpage::{
    ExtractedContent, capture_webpage_snapshot, extract_full_content, webpage_fallback_title,
};

pub(crate) fn prepare_display_html(
    content_html: Option<&str>,
    content_text: Option<&str>,
    summary: Option<&str>,
) -> Option<String> {
    if let Some(content_html) = content_html.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(sanitize_html_fragment(content_html));
    }

    if let Some(summary_html) = summary.map(str::trim).filter(|value| value.contains('<')) {
        return Some(sanitize_html_fragment(summary_html));
    }

    let text = content_text.map(str::trim).filter(|value| !value.is_empty())?;
    if looks_like_markdown(text) {
        return Some(markdown_to_safe_html(text));
    }
    None
}

pub(crate) fn clean_preview_text(value: &str) -> Option<String> {
    let plain = normalize_preview_whitespace(&strip_html_tags(value));
    if plain.is_empty() {
        return None;
    }
    Some(plain.chars().take(180).collect())
}

pub(crate) fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

pub(crate) fn normalize_preview_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn looks_like_markdown(text: &str) -> bool {
    if text.contains("```") || text.contains("~~~") || text.contains("<!--") || text.contains("![")
    {
        return true;
    }

    let mut markdown_signals = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if line.starts_with("    ") || line.starts_with('\t') {
            return true;
        }
        if trimmed.starts_with('#')
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed.starts_with("> ")
            || trimmed.starts_with("1. ")
            || trimmed.starts_with("2. ")
            || trimmed.starts_with("3. ")
        {
            markdown_signals += 1;
        }
        if trimmed.starts_with('|') && trimmed.contains('|') {
            markdown_signals += 1;
        }
    }

    markdown_signals >= 2 || text.contains("](") || text.contains("**") || text.contains("__")
}

pub(crate) fn markdown_to_safe_html(markdown: &str) -> String {
    let mut options = MarkdownOptions::empty();
    options.insert(MarkdownOptions::ENABLE_TABLES);
    options.insert(MarkdownOptions::ENABLE_STRIKETHROUGH);
    options.insert(MarkdownOptions::ENABLE_TASKLISTS);
    options.insert(MarkdownOptions::ENABLE_FOOTNOTES);

    let parser = MarkdownParser::new_ext(markdown, options);
    let mut rendered = String::new();
    markdown_html::push_html(&mut rendered, parser);
    sanitize_html_fragment(&rendered)
}

pub(crate) fn sanitize_html_fragment(html: &str) -> String {
    HtmlSanitizerBuilder::default()
        .add_tags([
            "article",
            "aside",
            "b",
            "br",
            "body",
            "div",
            "em",
            "section",
            "footer",
            "figure",
            "figcaption",
            "header",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "hr",
            "i",
            "img",
            "iframe",
            "li",
            "main",
            "nav",
            "ol",
            "p",
            "picture",
            "pre",
            "blockquote",
            "code",
            "small",
            "summary",
            "source",
            "span",
            "strong",
            "sub",
            "sup",
            "details",
            "table",
            "tbody",
            "td",
            "tfoot",
            "th",
            "thead",
            "time",
            "tr",
            "ul",
            "video",
        ])
        .add_tag_attributes("a", ["href", "title", "target"])
        .add_tag_attributes(
            "img",
            ["src", "alt", "title", "srcset", "sizes", "width", "height", "loading", "decoding"],
        )
        .add_tag_attributes("iframe", ["src", "title", "width", "height", "allow", "loading"])
        .add_tag_attributes(
            "video",
            ["src", "poster", "controls", "autoplay", "muted", "loop", "playsinline"],
        )
        .add_tag_attributes("source", ["src", "type", "media", "srcset", "sizes"])
        .add_tag_attributes("time", ["datetime"])
        .clean(html)
        .to_string()
}
