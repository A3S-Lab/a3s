export const pluginContentSecurityPolicy = [
  "default-src 'none'",
  "base-uri 'none'",
  "object-src 'none'",
  "frame-src 'none'",
  "child-src 'none'",
  "worker-src 'none'",
  "connect-src 'none'",
  "form-action 'none'",
  "navigate-to 'none'",
  'img-src data: blob:',
  'media-src data: blob:',
  'font-src data:',
  "style-src 'unsafe-inline'",
  "script-src 'unsafe-inline'",
].join('; ');

/**
 * Reparse package HTML before rendering so the host policy is the first item
 * in a real document head. DOMParser is deliberately used instead of string
 * insertion, which can be confused by comments or malformed markup.
 */
export function buildPluginDocument(html: string): string {
  const document = new DOMParser().parseFromString(html, 'text/html');
  document.querySelectorAll('meta[http-equiv]').forEach((element) => {
    if (element.getAttribute('http-equiv')?.toLowerCase() === 'refresh') element.remove();
  });
  document.querySelectorAll('base').forEach((element) => {
    element.remove();
  });

  const policy = document.createElement('meta');
  policy.setAttribute('http-equiv', 'Content-Security-Policy');
  policy.setAttribute('content', pluginContentSecurityPolicy);
  document.head.prepend(policy);

  return `<!doctype html>\n${document.documentElement.outerHTML}`;
}
