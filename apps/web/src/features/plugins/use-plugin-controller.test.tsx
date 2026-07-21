import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import { appState } from '../../state/app-state';
import type { PluginActivityCatalog, PluginActivityContent, PluginActivityItem } from '../../types/api';
import { createPluginsState } from './plugin-state';
import { usePluginController } from './use-plugin-controller';

const contribution: PluginActivityItem = {
  key: 'science:research',
  packageId: 'use/a3s/science',
  route: 'science',
  version: '1.2.3',
  enabled: true,
  id: 'research',
  title: 'Science',
  description: 'Explore scientific sources.',
  icon: 'flask-conical',
  skill: 'a3s-use-science',
  order: 120,
  sha256: 'a'.repeat(64),
  mediaType: 'text/html',
};

const catalog: PluginActivityCatalog = {
  schemaVersion: 1,
  available: true,
  generation: 2,
  revision: 'b'.repeat(64),
  items: [contribution],
};

describe('usePluginController', () => {
  beforeEach(() => {
    window.location.hash = '';
    Object.assign(appState, createPluginsState(), {
      activeProduct: 'code',
      pluginCatalog: catalog,
      pluginCatalogStatus: 'ready',
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.assign(appState, createPluginsState());
  });

  it('aborts the previous asset fetch and discards its late result', async () => {
    const first = createDeferred<PluginActivityContent>();
    const second = createDeferred<PluginActivityContent>();
    const signals: AbortSignal[] = [];
    vi.spyOn(codeApi, 'pluginActivities').mockResolvedValue(catalog);
    vi.spyOn(codeApi, 'pluginActivityContent')
      .mockImplementationOnce((_key, signal) => {
        signals.push(signal as AbortSignal);
        return first.promise;
      })
      .mockImplementationOnce((_key, signal) => {
        signals.push(signal as AbortSignal);
        return second.promise;
      });
    const hook = renderHook(() => usePluginController());

    let firstRequest!: Promise<void>;
    let secondRequest!: Promise<void>;
    act(() => {
      firstRequest = hook.result.current.loadActivityContent(contribution.key, true);
      secondRequest = hook.result.current.loadActivityContent(contribution.key, true);
    });

    expect(signals[0].aborted).toBe(true);
    second.resolve(activityContent('<p>current</p>'));
    await act(() => secondRequest);

    first.resolve(activityContent('<p>stale</p>'));
    await act(() => firstRequest);

    expect(appState.pluginContentByKey[contribution.key]?.html).toBe('<p>current</p>');
    expect(appState.pluginContentStatus).toBe('ready');
    hook.unmount();
  });
});

function activityContent(html: string): PluginActivityContent {
  return {
    key: contribution.key,
    packageId: contribution.packageId,
    skill: contribution.skill,
    registryRevision: catalog.revision,
    sha256: contribution.sha256,
    mediaType: 'text/html',
    html,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
