import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const livePreviewStyles = readFileSync(resolve(process.cwd(), 'src/styles/work-live-preview.css'), 'utf8');

describe('Work live preview layout', () => {
  it('keeps the complete PDF renderer chain at the panel height', () => {
    expect(livePreviewStyles).toMatch(/\.work-live-preview-document\s*\{[^}]*\n\s*height:\s*100%;/s);
    expect(livePreviewStyles).toMatch(
      /\.work-live-preview-document > \.work-pdf-viewer,\s*\.work-live-preview-document \.work-pdf-embed,\s*\.work-live-preview-document \.work-pdf-native-viewer,\s*\.work-live-preview-document embedpdf-container\s*\{[^}]*height:\s*100%;/s
    );
  });
});
