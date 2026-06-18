use std::time::Duration;

use foco_store::config::{
    WEB_SEARCH_PROVIDER_BRAVE, WEB_SEARCH_PROVIDER_TAVILY, WebSearchSettings,
};
use foco_tools::{WEB_FETCH_TOOL, WEB_SEARCH_TOOL};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_WEB_TOOL_TIMEOUT_MS: u64 = 15_000;
const MAX_WEB_TOOL_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_WEB_SEARCH_RESULT_LIMIT: usize = 5;
const MAX_WEB_SEARCH_RESULT_LIMIT: usize = 10;
const MAX_WEB_FETCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_WEB_FETCH_TEXT_CHARS: usize = 40_000;
const MAX_WEB_FETCH_RANGED_TEXT_CHARS: usize = 40_000;
const FOCO_WEB_USER_AGENT: &str = "Foco/0.1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebSearchToolInput {
    query: String,
    max_results: Option<usize>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WebFetchToolInput {
    url: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    timeout_ms: Option<u64>,
}

pub(crate) fn is_web_tool_name(tool_name: &str) -> bool {
    matches!(tool_name, WEB_SEARCH_TOOL | WEB_FETCH_TOOL)
}

pub(crate) fn web_tool_timeout_ms(arguments: &Value) -> Result<u64, String> {
    match arguments.get("timeoutMs") {
        Some(Value::Null) | None => Ok(DEFAULT_WEB_TOOL_TIMEOUT_MS),
        Some(Value::Number(timeout_ms)) => {
            let timeout_ms = timeout_ms
                .as_u64()
                .ok_or_else(|| "timeoutMs must be an integer or null".to_string())?;
            if timeout_ms == 0 || timeout_ms > MAX_WEB_TOOL_TIMEOUT_MS {
                Err(format!(
                    "timeoutMs must be between 1 and {MAX_WEB_TOOL_TIMEOUT_MS} milliseconds"
                ))
            } else {
                Ok(timeout_ms)
            }
        }
        Some(_) => Err("timeoutMs must be an integer or null".to_string()),
    }
}

pub(crate) async fn execute_web_tool(
    settings: &WebSearchSettings,
    tool_name: &str,
    arguments: Value,
    timeout: Duration,
) -> Result<Value, String> {
    match tool_name {
        WEB_SEARCH_TOOL => {
            let input = serde_json::from_value::<WebSearchToolInput>(arguments)
                .map_err(|source| format!("web_search arguments do not match schema: {source}"))?;
            execute_web_search(settings, input, timeout).await
        }
        WEB_FETCH_TOOL => {
            let input = serde_json::from_value::<WebFetchToolInput>(arguments)
                .map_err(|source| format!("web_fetch arguments do not match schema: {source}"))?;
            execute_web_fetch(input, timeout).await
        }
        _ => Err(format!("unknown web tool '{tool_name}'")),
    }
}

async fn execute_web_search(
    settings: &WebSearchSettings,
    input: WebSearchToolInput,
    timeout: Duration,
) -> Result<Value, String> {
    web_tool_timeout_ms_from_input(input.timeout_ms)?;
    if !web_search_enabled(settings) {
        return Err("web_search is disabled or missing an API key in settings".to_string());
    }
    let query = input.query.trim();
    if query.is_empty() {
        return Err("query must not be empty".to_string());
    }
    let max_results = normalize_web_search_limit(input.max_results)?;
    let provider = settings.active_provider.trim();
    let api_key = settings
        .api_key_for_provider(provider)
        .ok_or_else(|| format!("web_search provider '{provider}' is missing an API key"))?;
    let mut client_builder = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(FOCO_WEB_USER_AGENT);
    if settings.api_proxy.enabled {
        let proxy = reqwest::Proxy::all(settings.api_proxy.url.trim())
            .map_err(|source| format!("failed to configure web_search proxy: {source}"))?;
        client_builder = client_builder.proxy(proxy);
    }
    let client = client_builder
        .build()
        .map_err(|source| format!("failed to create web_search HTTP client: {source}"))?;
    let output = match provider {
        WEB_SEARCH_PROVIDER_TAVILY => tavily_search(&client, api_key, query, max_results).await?,
        WEB_SEARCH_PROVIDER_BRAVE => brave_search(&client, api_key, query, max_results).await?,
        other => return Err(format!("web_search provider '{other}' is unsupported")),
    };

    Ok(json!({
        "provider": provider,
        "query": query,
        "results": output,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64
    }))
}

async fn tavily_search(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<Value>, String> {
    let response = client
        .post("https://api.tavily.com/search")
        .bearer_auth(api_key)
        .json(&json!({
            "query": query,
            "max_results": max_results,
            "search_depth": "basic"
        }))
        .send()
        .await
        .map_err(|source| format!("Tavily search request failed: {source}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| format!("failed to read Tavily response: {source}"))?;
    if !status.is_success() {
        return Err(format_web_status_error("Tavily search", status, &body));
    }
    let value = serde_json::from_str::<Value>(&body)
        .map_err(|source| format!("failed to parse Tavily response JSON: {source}"))?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "Tavily response is missing results array".to_string())?;

    Ok(results
        .iter()
        .take(max_results)
        .map(|item| {
            json!({
                "title": item.get("title").and_then(Value::as_str).unwrap_or_default(),
                "url": item.get("url").and_then(Value::as_str).unwrap_or_default(),
                "snippet": item
                    .get("content")
                    .or_else(|| item.get("snippet"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "publishedAt": item
                    .get("published_date")
                    .or_else(|| item.get("publishedAt"))
                    .and_then(Value::as_str),
                "score": item.get("score").and_then(Value::as_f64)
            })
        })
        .collect())
}

async fn brave_search(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: usize,
) -> Result<Vec<Value>, String> {
    let mut url = reqwest::Url::parse("https://api.search.brave.com/res/v1/web/search")
        .map_err(|source| format!("invalid Brave search URL: {source}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("count", &max_results.to_string())
        .append_pair("text_decorations", "false");
    let response = client
        .get(url)
        .header("X-Subscription-Token", api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|source| format!("Brave search request failed: {source}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|source| format!("failed to read Brave response: {source}"))?;
    if !status.is_success() {
        return Err(format_web_status_error("Brave search", status, &body));
    }
    let value = serde_json::from_str::<Value>(&body)
        .map_err(|source| format!("failed to parse Brave response JSON: {source}"))?;
    let results = value
        .get("web")
        .and_then(|web| web.get("results"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Brave response is missing web.results array".to_string())?;

    Ok(results
        .iter()
        .take(max_results)
        .map(|item| {
            json!({
                "title": item.get("title").and_then(Value::as_str).unwrap_or_default(),
                "url": item.get("url").and_then(Value::as_str).unwrap_or_default(),
                "snippet": item
                    .get("description")
                    .or_else(|| item.get("snippet"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "publishedAt": item
                    .get("age")
                    .or_else(|| item.get("page_age"))
                    .and_then(Value::as_str),
                "score": null
            })
        })
        .collect())
}

async fn execute_web_fetch(input: WebFetchToolInput, timeout: Duration) -> Result<Value, String> {
    web_tool_timeout_ms_from_input(input.timeout_ms)?;
    let requested_line_range = parse_web_fetch_line_range(input.start_line, input.end_line)?;
    let url = parse_fetch_url(&input.url)?;
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(FOCO_WEB_USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|source| format!("failed to create web_fetch HTTP client: {source}"))?;
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|source| format!("web_fetch request failed: {source}"))?;
    let final_url = response.url().to_string();
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| String::new());
        return Err(format_web_status_error("web_fetch", status, &body));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WEB_FETCH_BYTES as u64)
    {
        return Err(format!(
            "web_fetch response is too large to read (max {MAX_WEB_FETCH_BYTES} bytes)"
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|source| format!("failed to read web_fetch response: {source}"))?;
    if bytes.len() > MAX_WEB_FETCH_BYTES {
        return Err(format!(
            "web_fetch response is too large to read ({} bytes; max {MAX_WEB_FETCH_BYTES})",
            bytes.len()
        ));
    }
    let raw_text = String::from_utf8_lossy(&bytes).to_string();
    let (title, text) = if content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("html")
    {
        (html_title(&raw_text), html_to_text(&raw_text))
    } else {
        (None, normalize_web_text(&raw_text))
    };
    let text = text.trim().to_string();
    let line_count = web_text_line_count(&text);
    let char_count = text.chars().count();
    let (text, start_line, end_line, truncated) = if let Some(range) = requested_line_range {
        let range = normalize_web_fetch_line_range(range, line_count)?;
        let ranged_text = web_text_line_range(&text, range);
        if ranged_text.chars().count() > MAX_WEB_FETCH_RANGED_TEXT_CHARS {
            return Err(format!(
                "web_fetch line range output is too large (max {MAX_WEB_FETCH_RANGED_TEXT_CHARS} characters); use a smaller line range"
            ));
        }
        (ranged_text, Some(range.0), Some(range.1), false)
    } else {
        if char_count > MAX_WEB_FETCH_TEXT_CHARS {
            return Err(format!(
                "web_fetch readable text is too large for a full read ({char_count} characters across {line_count} lines; max {MAX_WEB_FETCH_TEXT_CHARS}). Retry web_fetch with a smaller 1-based inclusive line range by setting startLine and endLine."
            ));
        }
        (text, None, None, false)
    };

    Ok(json!({
        "url": input.url,
        "finalUrl": final_url,
        "status": status.as_u16(),
        "contentType": content_type,
        "title": title,
        "text": text,
        "truncated": truncated,
        "bytes": bytes.len(),
        "lineCount": line_count,
        "startLine": start_line,
        "endLine": end_line,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64
    }))
}

pub(crate) fn web_search_enabled(settings: &WebSearchSettings) -> bool {
    settings.enabled
        && settings
            .api_key_for_provider(settings.active_provider.trim())
            .is_some()
}

fn normalize_web_search_limit(limit: Option<usize>) -> Result<usize, String> {
    let limit = limit.unwrap_or(DEFAULT_WEB_SEARCH_RESULT_LIMIT);
    if !(1..=MAX_WEB_SEARCH_RESULT_LIMIT).contains(&limit) {
        return Err(format!(
            "maxResults must be between 1 and {MAX_WEB_SEARCH_RESULT_LIMIT}"
        ));
    }

    Ok(limit)
}

fn web_tool_timeout_ms_from_input(timeout_ms: Option<u64>) -> Result<u64, String> {
    match timeout_ms {
        None => Ok(DEFAULT_WEB_TOOL_TIMEOUT_MS),
        Some(timeout_ms) if timeout_ms > 0 && timeout_ms <= MAX_WEB_TOOL_TIMEOUT_MS => {
            Ok(timeout_ms)
        }
        Some(_) => Err(format!(
            "timeoutMs must be between 1 and {MAX_WEB_TOOL_TIMEOUT_MS} milliseconds"
        )),
    }
}

fn parse_fetch_url(value: &str) -> Result<reqwest::Url, String> {
    let url =
        reqwest::Url::parse(value.trim()).map_err(|source| format!("invalid URL: {source}"))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(format!(
            "web_fetch only supports http and https URLs, got '{scheme}'"
        )),
    }
}

fn parse_web_fetch_line_range(
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<Option<(usize, usize)>, String> {
    match (start_line, end_line) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start > 0 && start <= end => Ok(Some((start, end))),
        (Some(_), Some(_)) => Err(
            "startLine and endLine must be a 1-based inclusive range with startLine <= endLine"
                .to_string(),
        ),
        _ => Err(
            "startLine and endLine must both be null for full-page fetches or both be integers for ranged fetches"
                .to_string(),
        ),
    }
}

fn normalize_web_fetch_line_range(
    range: (usize, usize),
    line_count: usize,
) -> Result<(usize, usize), String> {
    if line_count == 0 || range.0 > line_count {
        return Err(format!(
            "web_fetch line range {}-{} is outside the readable text; text has {line_count} lines",
            range.0, range.1
        ));
    }

    Ok((range.0, range.1.min(line_count)))
}

fn web_text_line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

fn web_text_line_range(text: &str, range: (usize, usize)) -> String {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            (line_number >= range.0 && line_number <= range.1)
                .then(|| format!("{line_number}\t{line}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_web_status_error(context: &str, status: reqwest::StatusCode, body: &str) -> String {
    let preview = body.trim();
    if preview.is_empty() {
        format!("{context} returned HTTP {status}")
    } else {
        let (preview, _) = truncate_chars(preview.to_string(), 800);
        format!("{context} returned HTTP {status}: {preview}")
    }
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let start = lower[start..].find('>').map(|offset| start + offset + 1)?;
    let end = lower[start..]
        .find("</title>")
        .map(|offset| start + offset)?;
    let title = html[start..end].trim();
    (!title.is_empty()).then(|| decode_basic_html_entities(title))
}

fn html_to_text(html: &str) -> String {
    let without_scripts = regex::Regex::new("(?is)<script\\b[^>]*>.*?</script>")
        .expect("valid script regex")
        .replace_all(html, " ");
    let without_styles = regex::Regex::new("(?is)<style\\b[^>]*>.*?</style>")
        .expect("valid style regex")
        .replace_all(&without_scripts, " ");
    let with_breaks = regex::Regex::new("(?i)<\\s*(br|p|div|li|h[1-6]|tr)\\b[^>]*>")
        .expect("valid block regex")
        .replace_all(&without_styles, "\n");
    let without_tags = regex::Regex::new("(?is)<[^>]+>")
        .expect("valid tag regex")
        .replace_all(&with_breaks, " ");
    normalize_web_text(&decode_basic_html_entities(&without_tags))
}

fn decode_basic_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn normalize_web_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_chars(value: String, max_chars: usize) -> (String, bool) {
    if value.chars().count() <= max_chars {
        return (value, false);
    }

    (value.chars().take(max_chars).collect(), true)
}
