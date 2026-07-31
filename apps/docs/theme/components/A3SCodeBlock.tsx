import { useMemo, useState } from "react";
import { useLang } from "@rspress/core/runtime";
import {
  InnerLine,
  Pre,
  type AnnotationHandler,
  type HighlightedCode,
} from "codehike/code";

const languageNames: Record<string, string> = {
  bash: "Shell",
  console: "Terminal",
  javascript: "JavaScript",
  js: "JavaScript",
  json: "JSON",
  markdown: "Markdown",
  md: "Markdown",
  python: "Python",
  py: "Python",
  rust: "Rust",
  shell: "Shell",
  text: "Text",
  ts: "TypeScript",
  tsx: "TSX",
  typescript: "TypeScript",
};

const lineNumbers: AnnotationHandler = {
  name: "line-numbers",
  Line: (props) => (
    <InnerLine
      className="a3s-doc-code-line"
      data-line-number={props.lineNumber}
      merge={props}
    />
  ),
};

const focus: AnnotationHandler = {
  name: "focus",
  onlyIfAnnotated: true,
  Line: (props) => (
    <InnerLine
      className="a3s-doc-code-line is-dimmed"
      data-line-number={props.lineNumber}
      merge={props}
    />
  ),
  AnnotatedLine: ({ annotation: _annotation, ...props }) => (
    <InnerLine
      className="a3s-doc-code-line is-focused"
      data-line-number={props.lineNumber}
      merge={props}
    />
  ),
};

function parseTitle(meta: string) {
  const match = meta.match(
    /(?:^|\s)(?:title|filename)=(?:"([^"]+)"|'([^']+)'|([^\s]+))/,
  );
  return match?.[1] ?? match?.[2] ?? match?.[3] ?? "";
}

export default function A3SCodeBlock({
  codeblock,
}: {
  codeblock: HighlightedCode;
}) {
  const isChinese = useLang() !== "en";
  const [copied, setCopied] = useState(false);
  const [wrapped, setWrapped] = useState(false);
  const { lineCount, longestLine } = useMemo(() => {
    const lines = codeblock.code.split("\n");
    return {
      lineCount: Math.max(lines.length, 1),
      longestLine: Math.max(...lines.map((line) => line.length), 0),
    };
  }, [codeblock.code]);
  const title = parseTitle(codeblock.meta);
  const language =
    languageNames[codeblock.lang.toLowerCase()] ?? codeblock.lang.toUpperCase();
  const compact = lineCount <= 3 && longestLine <= 96 && !title;

  async function copyCode() {
    await navigator.clipboard.writeText(codeblock.code);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <figure
      className={[
        "a3s-doc-code",
        compact ? "is-compact" : "",
        wrapped ? "is-wrapped" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <figcaption>
        <span className="a3s-doc-code-identity">
          <i aria-hidden="true" />
          <strong>{title || language}</strong>
          {title ? <small>{language}</small> : null}
        </span>
        <span className="a3s-doc-code-actions">
          {!compact ? (
            <button
              aria-pressed={wrapped}
              onClick={() => setWrapped((value) => !value)}
              type="button"
            >
              {isChinese ? "换行" : "Wrap"}
            </button>
          ) : null}
          <button onClick={copyCode} type="button">
            {copied
              ? isChinese
                ? "已复制"
                : "Copied"
              : isChinese
                ? "复制"
                : "Copy"}
          </button>
        </span>
      </figcaption>
      <div className="a3s-doc-code-viewport">
        <Pre code={codeblock} handlers={[lineNumbers, focus]} />
      </div>
    </figure>
  );
}
