import { afterEach, describe, expect, it } from 'vitest';
import { conversationHash, createCodeShellState, parseShellLocation } from './code-state';

describe('Work conversation routes', () => {
  afterEach(() => {
    window.history.replaceState(null, '', '#home');
  });

  it('treats an encoded session key as a canonical Work conversation route', () => {
    expect(conversationHash('session/with spaces')).toBe('#conversation/session%2Fwith%20spaces');
    expect(parseShellLocation('#conversation/session%2Fwith%20spaces')).toMatchObject({
      activeProduct: 'work',
      conversationSessionId: 'session/with spaces',
      settingsOpen: false,
      valid: true,
      workRoute: 'conversation',
    });
  });

  it('rejects empty and malformed conversation routes instead of leaking another task', () => {
    expect(parseShellLocation('#conversation/')).toMatchObject({
      conversationSessionId: null,
      valid: false,
      workRoute: 'home',
    });
    expect(parseShellLocation('#conversation/%E0%A4%A')).toMatchObject({
      conversationSessionId: null,
      valid: false,
      workRoute: 'home',
    });
  });

  it('hydrates the shell from a copied conversation URL', () => {
    window.history.replaceState(null, '', '#conversation/task-42');

    expect(createCodeShellState()).toMatchObject({
      activeProduct: 'work',
      conversationSessionId: 'task-42',
      settingsOpen: false,
      workRoute: 'conversation',
    });
  });
});
