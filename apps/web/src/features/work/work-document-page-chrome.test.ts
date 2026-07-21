import { describe, expect, it } from 'vitest';
import { sanitizeDocumentPageChromeHtml } from './work-document-page-chrome';

describe('Work document page chrome', () => {
  it('normalizes contenteditable font colors without retaining unsafe markup', () => {
    const html = sanitizeDocumentPageChromeHtml(
      '<p><font color="#c2410c">Alert</font><script>window.bad = true</script></p>'
    );
    const document = new DOMParser().parseFromString(html, 'text/html');

    expect(document.body.querySelector('font, script')).toBeNull();
    expect(document.body.querySelector('span')?.style.color).toBeTruthy();
    expect(document.body.textContent).toContain('Alert');
  });
});
