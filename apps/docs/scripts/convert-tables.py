#!/usr/bin/env python3
"""
Convert markdown tables in MDX files to fumadocs TypeTable components.

Table column mapping strategy:
- 1 col after key  → description
- 2 cols after key → type + description  (if header contains "type"/"method"/"binding")
                  → description + default (if header contains "default"/"value"/"behavior")
                  → type + description   (fallback)
- 3 cols after key → type + description + default
"""

import re
import sys
from pathlib import Path

IMPORT_LINE = "import { TypeTable } from 'fumadocs-ui/components/type-table';\n"


def parse_table(lines: list[str]) -> tuple[list[str], list[list[str]]] | None:
    """Parse a markdown table block into (headers, rows). Returns None if not a valid table."""
    if len(lines) < 3:
        return None
    # Must have separator line (line index 1)
    if not re.match(r"^\|[-| :]+\|?\s*$", lines[1]):
        return None

    def split_row(line: str) -> list[str]:
        line = line.strip().strip("|")
        return [cell.strip() for cell in line.split("|")]

    headers = split_row(lines[0])
    rows = [split_row(l) for l in lines[2:] if l.strip().startswith("|")]
    return headers, rows


def escape_mdx(s: str) -> str:
    """Escape backtick strings and angle brackets for JSX attribute values."""
    # Replace backtick code spans with <code> tags inside JSX strings
    # We'll use template literals to avoid escaping issues — wrap in {}
    return s


def to_jsx_key(s: str) -> str:
    """Convert a table key to a valid JSX object key (always a plain quoted string)."""
    s = s.strip()
    # Strip markdown formatting from keys
    s = re.sub(r"\*\*(.+?)\*\*", r"\1", s)  # bold
    s = re.sub(r"`(.+?)`", r"\1", s)         # inline code
    s = s.replace('"', '\\"')
    return f'"{s}"'


def to_jsx_string(s: str) -> str:
    """Convert a cell value to a safe JSX string attribute value."""
    s = s.strip()
    # Remove bold markdown
    s = re.sub(r"\*\*(.+?)\*\*", r"\1", s)
    # If contains backticks, angle brackets, or quotes, use JSX expression with template literal
    if "`" in s or "<" in s or ">" in s or "{" in s:
        s_escaped = s.replace("\\", "\\\\").replace("`", "\\`").replace("$", "\\$")
        return "{`" + s_escaped + "`}"
    # Plain string — just quote it
    s_escaped = s.replace('"', "&quot;")
    return f'"{s_escaped}"'


def headers_to_mapping(headers: list[str]) -> dict:
    """
    Given table headers, return a mapping of column index → TypeTable field name.
    Index 0 is always the key. Returns mapping for remaining columns.

    Strategy:
    - If headers clearly indicate type/description/default, map accordingly.
    - Otherwise, merge all extra columns into description (joined with " — ").
    """
    h = [col.lower().strip() for col in headers]
    n = len(h)

    if n == 2:
        return {1: "description"}

    if n == 3:
        h1, h2 = h[1], h[2]
        # Explicit type column
        if any(x in h1 for x in ["type"]):
            return {1: "type", 2: "description"}
        # Explicit default column
        if any(x in h2 for x in ["default"]):
            return {1: "description", 2: "default"}
        # Method/binding/protocol as type
        if any(x in h1 for x in ["method", "binding", "protocol", "format", "strategy", "returns"]):
            return {1: "type", 2: "description"}
        # Merge into description
        return {"merge": list(range(1, n))}

    if n == 4:
        h1, h2, h3 = h[1], h[2], h[3]
        if any(x in h1 for x in ["type"]) and any(x in h3 for x in ["default"]):
            return {1: "type", 2: "description", 3: "default"}
        if any(x in h3 for x in ["default"]):
            return {1: "description", 2: "type", 3: "default"}
        if any(x in h1 for x in ["type"]):
            return {1: "type", 2: "description", 3: "default"}
        # Merge all into description
        return {"merge": list(range(1, n))}

    # 5+ columns: merge all into description
    return {"merge": list(range(1, n))}


def row_to_type_node(key: str, row: list[str], mapping: dict, headers: list[str] | None = None) -> str:
    """Convert a table row to a TypeTable type node entry."""
    props = []

    if "merge" in mapping:
        # Merge specified columns into "Label: value" pairs
        parts = []
        for col_idx in mapping["merge"]:
            if col_idx < len(row):
                val = row[col_idx].strip()
                if val and val not in ("—", "-"):
                    if headers and col_idx < len(headers):
                        label = headers[col_idx].strip()
                        parts.append(f"{label}: {val}")
                    else:
                        parts.append(val)
        if parts:
            merged = " · ".join(parts)
            props.append(f"      description: {to_jsx_string(merged)},")
    else:
        for col_idx, field_name in sorted(mapping.items()):
            if field_name is None:
                continue
            if col_idx >= len(row):
                continue
            val = row[col_idx].strip()
            if not val or val in ("—", "-"):
                continue
            props.append(f"      {field_name}: {to_jsx_string(val)},")

    key_str = to_jsx_key(key)
    if not props:
        return f"    {key_str}: {{}},\n"
    return f"    {key_str}: {{\n" + "\n".join(props) + "\n    },\n"


def table_to_type_table(headers: list[str], rows: list[list[str]]) -> str:
    """Convert parsed table to TypeTable JSX."""
    mapping = headers_to_mapping(headers)
    entries = []
    for row in rows:
        if not row or not row[0]:
            continue
        key = row[0].strip()
        entries.append(row_to_type_node(key, row, mapping, headers))

    inner = "".join(entries)
    return f"<TypeTable\n  type={{\n{inner}  }}\n/>"


def find_table_blocks(lines: list[str]) -> list[tuple[int, int]]:
    """Find (start, end) line indices of all markdown table blocks."""
    blocks = []
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("|"):
            start = i
            while i < len(lines) and lines[i].startswith("|"):
                i += 1
            blocks.append((start, i))
        else:
            i += 1
    return blocks


def convert_file(path: Path) -> bool:
    """Convert all markdown tables in a file to TypeTable. Returns True if modified."""
    content = path.read_text(encoding="utf-8")
    lines = content.splitlines(keepends=True)

    # Find all table blocks (line indices)
    blocks = find_table_blocks([l.rstrip("\n") for l in lines])
    if not blocks:
        return False

    # Check if already has TypeTable import
    has_import = IMPORT_LINE.strip() in content

    # Process blocks in reverse order to preserve line indices
    modified = False
    for start, end in reversed(blocks):
        table_lines = [l.rstrip("\n") for l in lines[start:end]]
        parsed = parse_table(table_lines)
        if not parsed:
            continue
        headers, rows = parsed
        if not rows:
            continue

        jsx = table_to_type_table(headers, rows) + "\n"
        lines[start:end] = [jsx]
        modified = True

    if not modified:
        return False

    # Add import if needed
    if not has_import:
        # Find the last import line in frontmatter region or top of file
        new_lines = []
        inserted = False
        for line in lines:
            new_lines.append(line)
            # Insert after existing fumadocs imports, or after last import line
            if not inserted and line.strip().startswith("import ") and "fumadocs" in line:
                new_lines.append(IMPORT_LINE)
                inserted = True
        if not inserted:
            # Insert after frontmatter (after closing ---)
            result = []
            in_frontmatter = False
            fm_closed = False
            for line in new_lines:
                result.append(line)
                if line.strip() == "---" and not in_frontmatter:
                    in_frontmatter = True
                elif line.strip() == "---" and in_frontmatter and not fm_closed:
                    fm_closed = True
                    result.append("\n")
                    result.append(IMPORT_LINE)
                    inserted = True
            new_lines = result
        lines = new_lines

    path.write_text("".join(lines), encoding="utf-8")
    return True


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    mdx_files = sorted(root.rglob("*.mdx"))
    converted = 0
    for f in mdx_files:
        # Skip blog files
        if "blog" in f.parts:
            continue
        if convert_file(f):
            print(f"  ✓ {f.relative_to(root)}")
            converted += 1
    print(f"\nConverted {converted} files.")


if __name__ == "__main__":
    main()
