import { describe, expect, it } from 'vitest';
import { resolveAircraftConfiguration } from './aircraft-configuration';

describe('resolveAircraftConfiguration', () => {
  it('resolves model and effort along independent axes', () => {
    const configuration = resolveAircraftConfiguration({
      candidate: 'codex',
      model: 'zai/glm-5.2',
      effort: 'high',
    });

    expect(configuration.airframe.airframe.id).toBe('j-50');
    expect(configuration.loadout.id).toBe('heavy-air-to-air');
    expect(configuration.loadout.effort).toBe('high');
  });

  it('provides deterministic defaults without UI or runtime state', () => {
    const configuration = resolveAircraftConfiguration({});

    expect(configuration.airframe.airframe.id).toBe('prototype');
    expect(configuration.loadout.id).toBe('balanced-air-to-air');
  });

  it('lets an explicit hangar airframe override model-family selection', () => {
    const configuration = resolveAircraftConfiguration({
      airframeId: 'j-35',
      candidate: 'codex',
      model: 'openai/gpt-5.6',
      effort: 'low',
    });

    expect(configuration.airframe).toMatchObject({
      airframe: { id: 'j-35' },
      strategy: 'manual',
    });
    expect(configuration.loadout.effort).toBe('low');
  });
});
