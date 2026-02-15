---
name: browser-use
description: Deep web content extraction using AI-driven browser automation for pages that require rendering, interaction, or authentication
license: MIT
metadata:
  audience: researchers
  category: data-extraction
---

## Browser-Use: AI Browser Automation for Deep Research

You are an expert at using **browser-use** to extract content from web pages that cannot be accessed through simple HTTP requests or search APIs. This includes JavaScript-rendered pages, interactive web apps, paginated content, and sites that require navigation or form interaction.

### When to Use Browser Automation

Use browser-use **only** when standard search/fetch is insufficient:

- **JS-rendered content**: SPAs, React/Vue apps, dynamic dashboards
- **Paginated data**: Forum threads, search results spanning multiple pages
- **Interactive content**: Expandable sections, tabs, accordions, "load more" buttons
- **Authenticated content**: Sites requiring login (when credentials are provided)
- **PDF/document viewers**: Embedded documents rendered in-browser
- **Rate-limited APIs**: When direct API access is blocked but browser access works

Do NOT use browser-use for simple static pages — regular fetch is faster and cheaper.

### Setup

browser-use is a Python library. The A3S Code agent can invoke it via shell commands.

```bash
# Ensure browser-use is installed in the workspace
pip install browser-use playwright
playwright install chromium
```

### Core API Pattern

browser-use uses an `Agent` that controls a `Browser` instance with LLM-guided actions.

```python
#!/usr/bin/env python3
"""browser-use extraction script"""
import asyncio
import json
import sys
from browser_use import Agent, Browser, BrowserConfig
from langchain_openai import ChatOpenAI

async def extract(url: str, task: str) -> dict:
    browser = Browser(config=BrowserConfig(
        headless=True,
        disable_security=False,
    ))

    agent = Agent(
        task=task,
        llm=ChatOpenAI(model="gpt-4o-mini"),
        browser=browser,
        max_actions_per_step=5,
    )

    result = await agent.run(max_steps=15)
    await browser.close()

    return {
        "url": url,
        "content": result.final_result(),
        "steps": result.n_steps(),
    }

if __name__ == "__main__":
    url = sys.argv[1]
    task = sys.argv[2]
    data = asyncio.run(extract(url, task))
    print(json.dumps(data))
```

### Task Templates

When constructing browser-use tasks, be specific about what to extract:

**Extract article content:**
```
Go to {url}. Extract the main article content including title, author, date, and full text. If there are expandable sections, click to expand them all first. Return the content as structured text.
```

**Extract tabular data:**
```
Go to {url}. Find the data table on the page. If the table is paginated, navigate through all pages. Extract all rows and columns. Return as JSON array of objects.
```

**Extract from search results:**
```
Go to {url}. For each search result on the page, extract the title, URL, and snippet. If there are multiple pages, navigate to the first 3 pages. Return all results as JSON.
```

**Extract from interactive dashboard:**
```
Go to {url}. The page contains a dashboard with multiple tabs/sections. Click through each tab and extract the key metrics, charts data, and text content from each section. Return structured data.
```

**Extract PDF content:**
```
Go to {url}. The page contains an embedded PDF viewer. Extract the visible text content from the document. If the PDF has multiple pages, scroll through and extract all pages.
```

**Navigate and collect:**
```
Go to {url}. Find links to sub-pages about {topic}. Visit the top 5 most relevant sub-pages and extract their main content. Return a summary of each page.
```

### Integration with Research Pipeline

When the research agent encounters a URL that needs browser automation:

1. **Detect the need**: If a search result points to a JS-heavy site, SPA, or interactive page
2. **Write a Python script**: Generate a temporary `_browser_extract.py` in the workspace
3. **Execute via shell**: Run the script and capture JSON output
4. **Parse results**: Extract findings from the browser-use output
5. **Clean up**: Remove the temporary script

Example integration flow:

```bash
# Write extraction script
cat > /tmp/extract.py << 'EOF'
import asyncio, json, sys
from browser_use import Agent, Browser, BrowserConfig
from langchain_openai import ChatOpenAI

async def main():
    browser = Browser(config=BrowserConfig(headless=True))
    agent = Agent(
        task=sys.argv[2],
        llm=ChatOpenAI(model="gpt-4o-mini"),
        browser=browser,
        max_actions_per_step=5,
    )
    result = await agent.run(max_steps=10)
    await browser.close()
    print(json.dumps({"content": result.final_result()}))

asyncio.run(main())
EOF

# Execute
python3 /tmp/extract.py "https://example.com/dashboard" "Extract all metrics and data from this dashboard"
```

### Configuration

browser-use respects these environment variables:

- `OPENAI_API_KEY` — LLM for browser agent decisions (can also use `ANTHROPIC_API_KEY`)
- `BROWSER_USE_HEADLESS` — Set to `true` for headless mode (default in research)
- `BROWSER_USE_TIMEOUT` — Max seconds per extraction (default: 60)

### Guidelines

1. **Headless always**: Research runs in headless mode — no GUI needed
2. **Limit steps**: Set `max_steps=10-15` to prevent runaway browsing sessions
3. **Timeout**: Each extraction should complete within 60 seconds
4. **One page per task**: Don't try to extract from too many pages in a single agent run — split into multiple runs
5. **Structured output**: Always ask the browser agent to return JSON or structured text
6. **Error handling**: If extraction fails, fall back to regular search results — don't block the research pipeline
7. **Privacy**: Never store cookies, passwords, or session tokens in workspace files
8. **Resource cleanup**: Always close the browser instance after extraction

### Error Recovery

If browser-use fails (timeout, crash, blocked):

```python
try:
    result = await agent.run(max_steps=10)
except Exception as e:
    # Fall back to simple fetch
    import urllib.request
    response = urllib.request.urlopen(url)
    content = response.read().decode('utf-8')
    result = {"content": content, "fallback": True}
```

### When Invoked

You will receive a URL and a description of what content to extract. Follow this process:

1. Assess whether browser automation is actually needed (vs simple fetch)
2. Write a focused extraction task description
3. Generate and execute the Python script
4. Parse the JSON output into research findings
5. Return structured findings with source attribution

Output format:
```json
{
  "url": "https://example.com",
  "title": "Page Title",
  "extractedContent": "...",
  "keyPoints": ["point1", "point2"],
  "metadata": {
    "extractionMethod": "browser-use",
    "steps": 8,
    "duration": "12s"
  }
}
```
