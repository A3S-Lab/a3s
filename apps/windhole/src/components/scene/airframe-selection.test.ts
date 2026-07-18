import { describe, expect, it } from 'vitest';
import { AIRFRAMES, MODEL_AIRFRAME_RULES, resolveAirframe } from './airframe-selection';

describe('resolveAirframe', () => {
  it.each([
    ['GLM-5.2', 'j-50', 'model-exact'],
    ['GLM5.2', 'j-50', 'model-family'],
    ['zai/glm-5.2', 'j-50', 'model-exact'],
    ['GPT-5.6', 'f-35', 'model-exact'],
    ['GPT5.6', 'f-35', 'model-family'],
    ['openai/gpt-5.6', 'f-35', 'model-exact'],
  ] as const)('maps %s to %s', (model, airframeId, strategy) => {
    const resolution = resolveAirframe(model, 'claude-code');

    expect(resolution.airframe.id).toBe(airframeId);
    expect(resolution.strategy).toBe(strategy);
  });

  it.each([
    ['zhipu/glm-4.7-air', 'j-50'],
    ['openai/gpt-5.4', 'f-35'],
    ['openai/codex-mini-latest', 'f-35'],
    ['anthropic/claude-sonnet-4.6', 'f-22'],
  ] as const)('keeps current %s family models on a stable airframe', (model, airframeId) => {
    const resolution = resolveAirframe(model);

    expect(resolution.airframe.id).toBe(airframeId);
    expect(resolution.strategy).toBe('model-family');
  });

  it('lets an identified model override the Candidate fallback', () => {
    expect(resolveAirframe('glm-5.2', 'codex').airframe.id).toBe('j-50');
    expect(resolveAirframe('gpt-5.6', 'a3s-code').airframe.id).toBe('f-35');
  });

  it.each([
    ['', 'a3s-code', 'j-35'],
    ['vendor/new-model', 'claude-code', 'f-22'],
    ['vendor/new-model', './candidate', 'prototype'],
  ] as const)('falls back stably for model %s and Candidate %s', (model, candidate, airframeId) => {
    expect(resolveAirframe(model, candidate)).toMatchObject({
      airframe: { id: airframeId },
      strategy: candidate === './candidate' ? 'default' : 'candidate-fallback',
    });
  });

  it('exports immutable, enumerable airframe data and ordered rules', () => {
    expect(Object.keys(AIRFRAMES)).toEqual(['j-50', 'j-35', 'f-35', 'f-22', 'prototype']);
    expect(MODEL_AIRFRAME_RULES.map((rule) => rule.id)).toEqual(['glm', 'gpt', 'claude']);
    expect(Object.isFrozen(AIRFRAMES)).toBe(true);
    expect(Object.isFrozen(MODEL_AIRFRAME_RULES)).toBe(true);
  });
});
