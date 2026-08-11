import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, test } from 'node:test';
import { homeContent, type Lang } from './home-content';

describe('A3S CLI installation commands', () => {
  test('offers the tested cross-platform installers on both homepages', () => {
    for (const lang of ['cn', 'en'] satisfies Lang[]) {
      const installers = homeContent[lang].quickstart.installers;

      assert.deepEqual(installers.map((installer) => installer.id), ['unix', 'windows', 'homebrew']);
      assert.equal(installers[0].command.includes('/install.sh | sh'), true);
      assert.equal(installers[1].command.includes('/install.ps1 | iex'), true);
      assert.equal(installers[2].command.startsWith('brew install a3s-lab/tap/a3s'), true);
      assert.equal(installers.every((installer) => installer.command.includes('a3s code')), true);
    }
  });

  test('keeps an accessible copy label when mobile hides the visible text', () => {
    const source = readFileSync(new URL('./copy-command.tsx', import.meta.url), 'utf8');

    assert.equal(source.includes('aria-label={copied ? copiedLabel : copyLabel}'), true);
  });
});
