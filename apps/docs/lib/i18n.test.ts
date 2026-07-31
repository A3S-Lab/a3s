import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  defaultLocale,
  canonicalSitePath,
  localeFromPathname,
  localePath,
  localizedUrl,
  stripLocalePrefix,
} from '@/lib/i18n';

describe('documentation locale routing', () => {
  test('uses Chinese for unprefixed and legacy Chinese paths', () => {
    assert.equal(defaultLocale, 'cn');
    assert.equal(localeFromPathname('/docs/cloud'), 'cn');
    assert.equal(localeFromPathname('/cn/docs/cloud'), 'cn');
  });

  test('detects the prefixed English locale', () => {
    assert.equal(localeFromPathname('/en/docs/cloud'), 'en');
  });

  test('normalizes locale prefixes without losing query strings or fragments', () => {
    assert.equal(localePath('/cn/docs/cloud?tab=api#rest', 'en'),
      '/en/docs/cloud?tab=api#rest',
    );
    assert.equal(localePath('/en/docs/cloud?tab=api#rest', 'cn'),
      '/docs/cloud?tab=api#rest',
    );
    assert.equal(localePath('/', 'en'), '/en');
    assert.equal(localePath('/en', 'cn'), '/');
  });

  test('leaves external and protocol-relative links unchanged', () => {
    assert.equal(localePath('https://github.com/A3S-Lab', 'cn'),
      'https://github.com/A3S-Lab',
    );
    assert.equal(localePath('//cdn.example.com/a.js', 'en'), '//cdn.example.com/a.js');
  });

  test('builds canonical localized URLs', () => {
    assert.equal(localizedUrl('https://a3s.dev/', '/docs', 'cn'), 'https://a3s.dev/docs');
    assert.equal(localizedUrl('https://a3s.dev/', '/docs', 'en'),
      'https://a3s.dev/en/docs',
    );
  });

  test('uses canonical directory URLs for custom site links', () => {
    assert.equal(canonicalSitePath('/docs'), '/docs/');
    assert.equal(canonicalSitePath('/en/docs?tab=api#rest'), '/en/docs/?tab=api#rest');
    assert.equal(canonicalSitePath('/#architecture'), '/#architecture');
    assert.equal(canonicalSitePath('/favicon.svg'), '/favicon.svg');
    assert.equal(canonicalSitePath('https://github.com/A3S-Lab'), 'https://github.com/A3S-Lab');
  });

  test('strips only a leading supported locale', () => {
    assert.equal(stripLocalePrefix('/cn/docs/code'), '/docs/code');
    assert.equal(stripLocalePrefix('/docs/cn/code'), '/docs/cn/code');
  });
});
