import { type ReactNode, memo, useEffect, useState } from "react";
import { createHighlighter, type Highlighter } from "shiki";
import CopyButton from "../copy-button";

// =============================================================================
// Singleton highlighter — avoids re-loading WASM + themes per code block
// =============================================================================

let _highlighter: Highlighter | null = null;
let _highlighterPromise: Promise<Highlighter> | null = null;
const _loadedLangs = new Set<string>();

// Common languages to pre-load; others are loaded on demand
const PRELOAD_LANGS = [
	"javascript",
	"typescript",
	"jsx",
	"tsx",
	"json",
	"html",
	"css",
	"bash",
	"shell",
	"python",
	"rust",
	"toml",
	"yaml",
	"markdown",
	"sql",
	"diff",
];

function getHighlighter(): Promise<Highlighter> {
	if (_highlighter) return Promise.resolve(_highlighter);
	if (_highlighterPromise) return _highlighterPromise;
	_highlighterPromise = createHighlighter({
		themes: ["github-light", "github-dark"],
		langs: PRELOAD_LANGS,
	}).then((h) => {
		_highlighter = h;
		for (const lang of PRELOAD_LANGS) _loadedLangs.add(lang);
		return h;
	});
	return _highlighterPromise;
}

// =============================================================================
// LRU highlight cache — avoids re-highlighting identical code blocks
// =============================================================================

const MAX_CACHE = 128;
const _cache = new Map<string, string>();

function cacheKey(code: string, lang: string): string {
	return `${lang}:${code}`;
}

function cacheGet(key: string): string | undefined {
	const val = _cache.get(key);
	if (val !== undefined) {
		// Move to end (most recently used)
		_cache.delete(key);
		_cache.set(key, val);
	}
	return val;
}

function cacheSet(key: string, val: string): void {
	if (_cache.size >= MAX_CACHE) {
		// Evict oldest entry
		const first = _cache.keys().next().value;
		if (first !== undefined) _cache.delete(first);
	}
	_cache.set(key, val);
}

async function highlight(code: string, lang: string): Promise<string> {
	const key = cacheKey(code, lang);
	const cached = cacheGet(key);
	if (cached) return cached;

	const highlighter = await getHighlighter();

	// Lazy-load unknown languages
	if (!_loadedLangs.has(lang)) {
		try {
			await highlighter.loadLanguage(
				lang as Parameters<Highlighter["loadLanguage"]>[0],
			);
			_loadedLangs.add(lang);
		} catch {
			// Unknown language — fall back to "text"
			if (!_loadedLangs.has("text")) {
				await highlighter.loadLanguage("text");
				_loadedLangs.add("text");
			}
			const html = highlighter.codeToHtml(code, {
				lang: "text",
				themes: { light: "github-light", dark: "github-dark" },
			});
			cacheSet(key, html);
			return html;
		}
	}

	const html = highlighter.codeToHtml(code, {
		lang,
		themes: { light: "github-light", dark: "github-dark" },
	});
	cacheSet(key, html);
	return html;
}

// =============================================================================
// Component
// =============================================================================

interface CodeHighlightProps {
	className?: string;
	children?: ReactNode;
}

const CodeHighlight = memo(
	({ className, children, ...props }: CodeHighlightProps) => {
		const code = String(children).trim();
		const language = className?.match(/language-(\w+)/)?.[1] || "text";
		const isInline = !className;

		const [html, setHtml] = useState<string>(() => {
			// Synchronous cache hit — avoids flash of unstyled code
			if (isInline) return "";
			return cacheGet(cacheKey(code, language)) || "";
		});

		useEffect(() => {
			if (isInline) return;
			// If we already have a sync cache hit, skip
			const key = cacheKey(code, language);
			if (cacheGet(key)) {
				setHtml(cacheGet(key)!);
				return;
			}
			let cancelled = false;
			highlight(code, language).then((result) => {
				if (!cancelled) setHtml(result);
			});
			return () => {
				cancelled = true;
			};
		}, [code, language, isInline]);

		if (isInline) {
			return (
				<code
					className="rounded bg-muted px-1.5 py-0.5 text-[13px] font-mono"
					{...props}
				>
					{children}
				</code>
			);
		}

		return (
			<div className="relative group">
				<div className="shiki-header">
					<span className="language-label text-xs text-muted-foreground">
						{language}
					</span>
				</div>
				<div className="shiki-code">
					{html ? (
						<div dangerouslySetInnerHTML={{ __html: html }} />
					) : (
						<pre className={className} {...props}>
							<code>{children}</code>
						</pre>
					)}
				</div>
				<CopyButton text={code} />
			</div>
		);
	},
	(prev, next) =>
		prev.className === next.className &&
		String(prev.children).trim() === String(next.children).trim(),
);

CodeHighlight.displayName = "CodeHighlight";

export default CodeHighlight;
