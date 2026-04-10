use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use scraper::{Html, Selector};

use super::ExecutionAdapter;
use crate::{CapabilityRisk, ExecutionCapability, ExecutionCapabilityGrant};

pub struct HttpExecutionAdapter;

#[async_trait]
impl ExecutionAdapter for HttpExecutionAdapter {
    fn capabilities(&self) -> Vec<ExecutionCapabilityGrant> {
        vec![
            ExecutionCapabilityGrant {
                capability: ExecutionCapability::Network {
                    protocol: "http".into(),
                    operation: "fetch".into(),
                    scope: "public".into(),
                },
                risk: CapabilityRisk::Low,
            },
            ExecutionCapabilityGrant {
                capability: ExecutionCapability::Network {
                    protocol: "http".into(),
                    operation: "post".into(),
                    scope: "public".into(),
                },
                risk: CapabilityRisk::Medium,
            },
            ExecutionCapabilityGrant {
                capability: ExecutionCapability::Network {
                    protocol: "http".into(),
                    operation: "extract".into(),
                    scope: "public".into(),
                },
                risk: CapabilityRisk::Medium,
            },
        ]
    }

    async fn execute(
        &self,
        handler: &str,
        input: &serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        execute(handler, input, timeout).await
    }
}

pub async fn execute(
    handler: &str,
    input: &serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    match handler {
        "get" => {
            let request = HttpGetInput::from_value(input)?;
            http_get(&request.url, &request.headers, timeout).await
        }
        "post" => {
            let request = HttpPostInput::from_value(input)?;
            http_post(&request.url, &request.headers, &request.body, timeout).await
        }
        "extract" => {
            let request = HttpGetInput::from_value(input)?;
            web_extract(&request.url, &request.headers, timeout).await
        }
        other => Err(format!("unsupported http handler: {other}")),
    }
}

/// Execute HTTP GET request
pub async fn http_get(
    url: &str,
    headers: &HashMap<String, String>,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("failed to build http client: {e}"))?;

    let mut req = client.get(url);
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status().as_u16();
    let resp_headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
        .collect();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("failed to read body: {e}"))?;

    Ok(serde_json::json!({
        "status": status,
        "headers": resp_headers,
        "body": body,
    }))
}

/// Fetch a webpage and extract readable content for agent consumption.
pub async fn web_extract(
    url: &str,
    headers: &HashMap<String, String>,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let response = http_get(url, headers, timeout).await?;
    let status = response
        .get("status")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let body = response
        .get("body")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "missing body in fetched response".to_string())?;

    let extracted = extract_html_content(body);

    Ok(serde_json::json!({
        "status": status,
        "url": url,
        "title": extracted.title,
        "description": extracted.description,
        "content_text": extracted.content_text,
        "content_html": extracted.content_html,
        "links": extracted.links,
    }))
}

/// Execute HTTP POST request
pub async fn http_post(
    url: &str,
    headers: &HashMap<String, String>,
    body: &str,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("failed to build http client: {e}"))?;

    let mut req = client.post(url).body(body.to_string());
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status().as_u16();
    let resp_headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_string())))
        .collect();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("failed to read body: {e}"))?;

    Ok(serde_json::json!({
        "status": status,
        "headers": resp_headers,
        "body": body,
    }))
}

#[derive(Debug)]
struct HttpGetInput {
    url: String,
    headers: HashMap<String, String>,
}

impl HttpGetInput {
    fn from_value(input: &serde_json::Value) -> Result<Self, String> {
        let url = input
            .get("url")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "http input requires string field `url`".to_string())?
            .to_string();
        let headers = input
            .get("headers")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| format!("invalid http headers: {e}"))?
            .unwrap_or_default();

        Ok(Self { url, headers })
    }
}

#[derive(Debug)]
struct HttpPostInput {
    url: String,
    headers: HashMap<String, String>,
    body: String,
}

impl HttpPostInput {
    fn from_value(input: &serde_json::Value) -> Result<Self, String> {
        let base = HttpGetInput::from_value(input)?;
        let body = input
            .get("body")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "http post input requires string field `body`".to_string())?
            .to_string();

        Ok(Self {
            url: base.url,
            headers: base.headers,
            body,
        })
    }
}

#[derive(Debug)]
struct ExtractedContent {
    title: Option<String>,
    description: Option<String>,
    content_text: String,
    content_html: Option<String>,
    links: Vec<String>,
}

fn extract_html_content(html: &str) -> ExtractedContent {
    let document = Html::parse_document(html);
    let title = select_first_text(&document, "title");
    let description = select_meta_content(&document, "description")
        .or_else(|| select_meta_property(&document, "og:description"));
    let content_html =
        select_first_html(&document, "article").or_else(|| select_first_html(&document, "main"));
    let content_text = content_html
        .as_deref()
        .map(extract_text_fragment)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| extract_text_fragment(html));
    let links = collect_links(&document);

    ExtractedContent {
        title,
        description,
        content_text,
        content_html,
        links,
    }
}

fn select_first_text(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| normalize_text(&element.text().collect::<Vec<_>>().join(" ")))
        .filter(|text| !text.is_empty())
}

fn select_first_html(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()
        .map(|element| element.html())
}

fn select_meta_content(document: &Html, name: &str) -> Option<String> {
    let selector = Selector::parse(&format!("meta[name=\"{name}\"]")).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .map(normalize_text)
        .filter(|text| !text.is_empty())
}

fn select_meta_property(document: &Html, property: &str) -> Option<String> {
    let selector = Selector::parse(&format!("meta[property=\"{property}\"]")).ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .map(normalize_text)
        .filter(|text| !text.is_empty())
}

fn collect_links(document: &Html) -> Vec<String> {
    let Ok(selector) = Selector::parse("a[href]") else {
        return Vec::new();
    };

    document
        .select(&selector)
        .filter_map(|element| element.value().attr("href"))
        .map(normalize_text)
        .filter(|href| !href.is_empty())
        .take(100)
        .collect()
}

fn extract_text_fragment(fragment: &str) -> String {
    let fragment = Html::parse_fragment(fragment);
    normalize_text(&fragment.root_element().text().collect::<Vec<_>>().join(" "))
}

fn normalize_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{execute, extract_html_content};
    use std::time::Duration;

    #[test]
    fn extracts_article_like_content() {
        let html = r#"
        <html>
          <head>
            <title>Test Page</title>
            <meta name="description" content="Summary text" />
          </head>
          <body>
            <nav><a href="/home">Home</a></nav>
            <main>
              <h1>Hello</h1>
              <p>Structured page content.</p>
              <a href="https://example.com/source">Source</a>
            </main>
          </body>
        </html>
        "#;

        let extracted = extract_html_content(html);

        assert_eq!(extracted.title.as_deref(), Some("Test Page"));
        assert_eq!(extracted.description.as_deref(), Some("Summary text"));
        assert!(extracted.content_text.contains("Structured page content."));
        assert!(extracted.links.iter().any(|link| link == "/home"));
        assert!(extracted
            .links
            .iter()
            .any(|link| link == "https://example.com/source"));
    }

    #[tokio::test]
    async fn rejects_unknown_handler() {
        let err = execute("unknown", &serde_json::json!({}), Duration::from_secs(1))
            .await
            .expect_err("unknown handler should fail");
        assert!(err.contains("unsupported http handler"));
    }
}
