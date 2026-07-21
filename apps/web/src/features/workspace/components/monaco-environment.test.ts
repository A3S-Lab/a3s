import { describe, expect, it } from 'vitest';
import { languageForPath } from './monaco-environment';

describe('Monaco environment', () => {
  it('loads the Chinese message catalog used by the native editor context menu', () => {
    const messages = (globalThis as typeof globalThis & { _VSCODE_NLS_MESSAGES?: string[] })._VSCODE_NLS_MESSAGES;

    expect(messages).toEqual(expect.arrayContaining(['剪切', '复制', '粘贴', '命令面板']));
  });

  it.each([
    ['README.md', 'markdown'],
    ['src/app.tsx', 'typescript'],
    ['src/main.rs', 'rust'],
    ['agent.acl', 'a3s-acl'],
  ])('maps %s to the %s language', (path, language) => {
    expect(languageForPath(path)).toBe(language);
  });
});
