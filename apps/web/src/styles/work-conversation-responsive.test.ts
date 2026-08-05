import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const conversationStyles = readFileSync(resolve(process.cwd(), 'src/styles/work-conversation.css'), 'utf8');

describe('Work conversation responsive styles', () => {
  it('responds to the conversation pane after the activity bar and task library take space', () => {
    expect(conversationStyles).toMatch(/\.work-conversation\s*\{[^}]*container:\s*work-conversation \/ inline-size;/s);
    expect(conversationStyles).toMatch(
      /@container work-conversation \(max-width:\s*760px\)[\s\S]*?\.work-conversation-header\s*\{[^}]*flex-direction:\s*column;/
    );
  });

  it('keeps compact actions usable and preserves a readable thread on narrow panes', () => {
    expect(conversationStyles).toMatch(
      /@container work-conversation \(max-width:\s*430px\)[\s\S]*?\.work-conversation-actions \.ds-button\s*\{[^}]*min-width:\s*32px;/
    );
    expect(conversationStyles).toMatch(
      /@container work-conversation \(max-width:\s*430px\)[\s\S]*?\.work-conversation \.execution-column\s*\{[^}]*width:\s*calc\(100% - 28px\);/
    );
  });

  it('removes the authored route transition when reduced motion is requested', () => {
    expect(conversationStyles).toMatch(
      /@media \(prefers-reduced-motion:\s*reduce\)[\s\S]*?\.work-conversation-body\s*\{[^}]*animation:\s*none;/
    );
  });
});
