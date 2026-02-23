import { marked } from "marked";
import { memo, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import CodeHighlight from "./code-highlight";
import "./index.css";

// =============================================================================
// Stable plugin arrays — hoisted to module scope to avoid re-creating on render
// =============================================================================

const REMARK_PLUGINS = [remarkGfm];
const REHYPE_PLUGINS = [rehypeRaw];

// =============================================================================
// Custom components — stable reference to avoid ReactMarkdown re-init
// =============================================================================

const MARKDOWN_COMPONENTS = {
	code: CodeHighlight,
	// External links open in new tab
	a: ({
		href,
		children,
		...props
	}: React.AnchorHTMLAttributes<HTMLAnchorElement>) => {
		const isExternal =
			href && (href.startsWith("http://") || href.startsWith("https://"));
		return (
			<a
				href={href}
				{...(isExternal
					? { target: "_blank", rel: "noopener noreferrer" }
					: {})}
				{...props}
			>
				{children}
			</a>
		);
	},
} as const;

// =============================================================================
// Block parsing — split markdown into top-level blocks for granular memoization
// =============================================================================

function parseMarkdownIntoBlocks(markdown: string): string[] {
	const tokens = marked.lexer(markdown);
	return tokens.map((token) => token.raw);
}

// =============================================================================
// MemoizedMarkdownBlock — renders a single markdown block
// =============================================================================

const MemoizedMarkdownBlock = memo(
	({ content }: { content: string }) => {
		return (
			<ReactMarkdown
				remarkPlugins={REMARK_PLUGINS}
				rehypePlugins={REHYPE_PLUGINS}
				components={MARKDOWN_COMPONENTS}
			>
				{content}
			</ReactMarkdown>
		);
	},
	(prev, next) => prev.content === next.content,
);

MemoizedMarkdownBlock.displayName = "MemoizedMarkdownBlock";

// =============================================================================
// MemoizedMarkdown — top-level component with block-level memoization
// =============================================================================

export const MemoizedMarkdown = memo(
	({ content, id }: { content: string; id: string }) => {
		const blocks = useMemo(() => parseMarkdownIntoBlocks(content), [content]);

		return (
			<div key={id} className="markdown">
				{blocks.map((block, index) => (
					<MemoizedMarkdownBlock content={block} key={`${id}-block_${index}`} />
				))}
			</div>
		);
	},
);

MemoizedMarkdown.displayName = "MemoizedMarkdown";
