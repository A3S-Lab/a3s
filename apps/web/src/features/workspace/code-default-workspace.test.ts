import { describe, expect, it } from 'vitest';
import { codeDefaultWorkspace } from './code-default-workspace';

describe('A3S Code default workspace', () => {
  it('prefers the persisted new-task workspace over runtime fallbacks', () => {
    expect(
      codeDefaultWorkspace({
        newTaskWorkspace: ' /clients/acme ',
        serviceWorkspace: '/service-default',
        currentWorkspace: '/active-task',
      })
    ).toBe('/clients/acme');
  });

  it('falls back to the service workspace before the current transient workspace', () => {
    expect(codeDefaultWorkspace({ serviceWorkspace: '/service-default', currentWorkspace: '/active-task' })).toBe(
      '/service-default'
    );
  });
});
