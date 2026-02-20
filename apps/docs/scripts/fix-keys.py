#!/usr/bin/env python3
"""Fix broken TypeTable computed key syntax.

Bad:  {`\`some-key\``}: {
      {`\`exec\` (buffered)`}: {
Good: "some-key": {
      "exec (buffered)": {
"""
import re
from pathlib import Path

# Match any {`...`}: used as an object key (at start of line with leading whitespace)
# These are computed keys which are invalid in JSX object literals
BAD_KEY = re.compile(r'^(\s+)\{`(.*?)`\}:', re.MULTILINE)


def fix_key(m: re.Match) -> str:
    indent = m.group(1)
    inner = m.group(2)
    # Strip escaped backticks: \` → nothing (remove backtick markdown)
    inner = inner.replace("\\`", "")
    # Escape any double quotes
    inner = inner.replace('"', '\\"')
    return f'{indent}"{inner}":'


root = Path("content/docs")
fixed = 0
for f in sorted(root.rglob("*.mdx")):
    text = f.read_text(encoding="utf-8")
    if "{`" not in text:
        continue
    new_text = BAD_KEY.sub(fix_key, text)
    if new_text != text:
        f.write_text(new_text, encoding="utf-8")
        print(f"  ✓ {f}")
        fixed += 1

print(f"\nFixed {fixed} files.")
