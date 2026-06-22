use std::time::Duration;

use shared_api::constants::{DEFAULT_TIMEOUT_SECS, USER_AGENT};
use shared_api::webpage::{WebpageCapture, sanitize_html_fragment};
use url::Url;

pub fn capture_webpage_snapshot(
    runtime: &tokio::runtime::Runtime,
    url: &Url,
) -> Result<WebpageCapture, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|err| format!("failed to initialize webpage client: {err}"))?;

    let mut capture = runtime
        .block_on(shared_api::webpage::capture_webpage_snapshot(&client, url))
        .map_err(|err| err.to_string())?;

    capture.content_html = capture.content_html.as_deref().map(sanitize_html_fragment);
    Ok(capture)
}
