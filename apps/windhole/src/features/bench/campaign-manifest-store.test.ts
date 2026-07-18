import { beforeEach, describe, expect, it } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { createRunCampaignSnapshot, isCampaignActive, type LabCampaignState, labState } from '../../state/lab-state';
import type { BenchRunResult } from '../../types/bench';
import type { HangarRosterEntry } from '../hangar/hangar-configuration';
import {
  CAMPAIGN_MANIFEST_STORAGE_KEY,
  restoreCampaignManifest,
  startCampaignManifestPersistence,
} from './campaign-manifest-store';

class MemoryStorage {
  readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

beforeEach(() => {
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  labState.runConfig.deploymentScope = 'single';
  labState.runConfig.locked = false;
  labState.catalog.selectedTaskId = 'another-map';
});

describe('campaign manifest store', () => {
  it('restores a complete frozen campaign group with exact per-member Run ID result caches', () => {
    const storage = new MemoryStorage();
    labState.campaign = completedCampaign(3);
    const expectedTask = labState.campaign.snapshot?.task;
    const expectedInputs = labState.campaign.snapshot?.roster.map((sortie) => sortie.input);
    const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T01:00:00.000Z');
    stop();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    labState.runConfig.locked = true;
    expect(restoreCampaignManifest(storage)).toBe(true);

    expect(labState.campaign).toMatchObject({
      generation: 7,
      status: 'completed',
      startedAt: '2026-07-17T00:00:00.000Z',
      completedAt: '2026-07-17T00:10:00.000Z',
    });
    expect(labState.runConfig).toMatchObject({ deploymentScope: 'campaign', locked: false });
    expect(labState.catalog.selectedTaskId).toBe(expectedTask?.id);
    expect(labState.campaign.snapshot?.task).toEqual(expectedTask);
    expect(labState.campaign.snapshot?.roster.map((sortie) => sortie.input)).toEqual(expectedInputs);
    expect(Object.isFrozen(labState.campaign.snapshot)).toBe(true);
    expect(Object.isFrozen(labState.campaign.snapshot?.task)).toBe(true);
    expect(Object.isFrozen(labState.campaign.snapshot?.roster)).toBe(true);
    expect(labState.campaign.members).toHaveLength(3);
    for (const [index, member] of labState.campaign.members.entries()) {
      expect(member).toMatchObject({
        rosterEntryId: `roster-${index}`,
        jobId: `job-${index}`,
        runId: `run-${index}`,
        status: 'completed',
        result: {
          run_id: `run-${index}`,
          task_id: demoTasks[0].id,
          score: `0.${index + 6}`,
          model_usage: { tool_calls_count: index + 1 },
        },
      });
      expect(member.sortie).toStrictEqual(labState.campaign.snapshot?.roster[index]);
      expect(member.result?.run_id).toBe(member.runId);
    }
  });

  it('round-trips nullable optional model usage for each completed campaign result', () => {
    const storage = new MemoryStorage();
    labState.campaign = completedCampaign(1);
    labState.campaign.members[0].result = completedResultWithNullableUsage(0);
    const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T01:00:00.000Z');
    stop();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    expect(restoreCampaignManifest(storage)).toBe(true);
    expect(labState.campaign.members[0].result?.model_usage).toEqual({
      prompt_tokens: 100,
      completion_tokens: 20,
      total_tokens: 120,
      cache_read_tokens: null,
      cache_write_tokens: null,
      tool_calls_count: null,
    });
  });

  it('turns interrupted frontend tracking into tracking_stopped without losing submitted Job and Run ownership', () => {
    const storage = new MemoryStorage();
    labState.campaign = runningCampaign();
    const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T00:03:00.000Z');
    stop();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    const restoredAt = '2026-07-17T00:04:00.000Z';
    expect(restoreCampaignManifest(storage, () => restoredAt)).toBe(true);

    expect(labState.campaign.status).toBe('tracking_stopped');
    expect(labState.campaign.completedAt).toBe(restoredAt);
    expect(isCampaignActive()).toBe(false);
    expect(labState.campaign.members.map((member) => member.status)).toEqual([
      'completed',
      'tracking_stopped',
      'tracking_stopped',
    ]);
    expect(labState.campaign.members[0]).toMatchObject({ runId: 'run-0', result: { run_id: 'run-0' } });
    expect(labState.campaign.members[1]).toMatchObject({
      jobId: 'job-1',
      stage: 'running',
      completedAt: restoredAt,
    });
    expect(labState.campaign.members[2]).toMatchObject({ completedAt: restoredAt });
    expect(labState.campaign.members[2].jobId).toBeUndefined();
  });

  it('preserves exact Run attribution while completed Jobs are still fetching their full reports', () => {
    const storage = new MemoryStorage();
    labState.campaign = runningCampaign();
    Object.assign(labState.campaign.members[1], {
      status: 'running',
      stage: 'completed',
      runId: 'run-1',
      completedAt: '2026-07-17T00:03:00.000Z',
    });
    Object.assign(labState.campaign.members[2], {
      status: 'starting',
      stage: 'completed',
      jobId: 'job-2',
      runId: 'run-2',
      startedAt: '2026-07-17T00:02:30.000Z',
      completedAt: '2026-07-17T00:03:01.000Z',
    });
    const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T00:03:02.000Z');
    stop();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    const restoredAt = '2026-07-17T00:04:00.000Z';
    expect(restoreCampaignManifest(storage, () => restoredAt)).toBe(true);

    expect(labState.campaign.status).toBe('tracking_stopped');
    expect(labState.campaign.members[1]).toMatchObject({
      status: 'tracking_stopped',
      stage: 'completed',
      jobId: 'job-1',
      runId: 'run-1',
      completedAt: '2026-07-17T00:03:00.000Z',
    });
    expect(labState.campaign.members[2]).toMatchObject({
      status: 'tracking_stopped',
      stage: 'completed',
      jobId: 'job-2',
      runId: 'run-2',
      completedAt: '2026-07-17T00:03:01.000Z',
    });
    expect(labState.campaign.members[1].result).toBeUndefined();
    expect(labState.campaign.members[2].result).toBeUndefined();
  });

  it('observes both campaign replacement and later nested member transitions', async () => {
    const storage = new MemoryStorage();
    const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T00:03:00.000Z');

    labState.campaign = runningCampaign();
    await Promise.resolve();
    expect(storage.getItem(CAMPAIGN_MANIFEST_STORAGE_KEY)).not.toBeNull();

    labState.campaign.members[1].stage = 'failed';
    labState.campaign.members[1].status = 'failed';
    labState.campaign.members[1].completedAt = '2026-07-17T00:03:30.000Z';
    labState.campaign.members[1].error = 'Candidate failed';
    labState.campaign.members[2].status = 'failed';
    labState.campaign.members[2].completedAt = '2026-07-17T00:03:31.000Z';
    labState.campaign.members[2].error = 'Campaign stopped';
    labState.campaign.status = 'completed_with_failures';
    labState.campaign.completedAt = '2026-07-17T00:03:31.000Z';
    labState.campaign.error = 'Two aircraft failed';
    await Promise.resolve();
    stop();

    labState.campaign = { generation: 0, status: 'idle', members: [] };
    expect(restoreCampaignManifest(storage)).toBe(true);
    expect(labState.campaign.status).toBe('completed_with_failures');
    expect(labState.campaign.members.map((member) => member.status)).toEqual(['completed', 'failed', 'failed']);
    expect(labState.campaign.members[1].error).toBe('Candidate failed');
  });

  it('rejects duplicate identities, cross-member result attribution, extra fields, and snapshot/member mismatch', () => {
    const mutations: Array<(manifest: Record<string, unknown>) => void> = [
      (manifest) => {
        const members = campaignRecord(manifest).members as Array<Record<string, unknown>>;
        members[1].jobId = members[0].jobId;
      },
      (manifest) => {
        const members = campaignRecord(manifest).members as Array<Record<string, unknown>>;
        members[1].runId = members[0].runId;
        members[1].result = members[0].result;
      },
      (manifest) => {
        const members = campaignRecord(manifest).members as Array<Record<string, unknown>>;
        (members[0].result as Record<string, unknown>).run_id = 'another-run';
      },
      (manifest) => {
        const members = campaignRecord(manifest).members as Array<Record<string, unknown>>;
        members[0].rosterEntryId = 'roster-1';
      },
      (manifest) => {
        campaignRecord(manifest).untrusted = true;
      },
    ];

    for (const mutate of mutations) {
      const storage = storedCompletedCampaign();
      const manifest = JSON.parse(storage.getItem(CAMPAIGN_MANIFEST_STORAGE_KEY) as string) as Record<string, unknown>;
      mutate(manifest);
      storage.setItem(CAMPAIGN_MANIFEST_STORAGE_KEY, JSON.stringify(manifest));
      labState.campaign = { generation: 0, status: 'idle', members: [] };

      expect(restoreCampaignManifest(storage)).toBe(false);
      expect(labState.campaign).toEqual({ generation: 0, status: 'idle', members: [] });
    }
  });

  it('rejects unsupported versions, oversized payloads, member overflows, and malformed timestamps', () => {
    const invalidPayloads: unknown[] = [
      { version: 2, savedAt: '2026-07-17T00:00:00.000Z', campaign: {} },
      { version: 1, savedAt: 'not-a-time', campaign: {} },
    ];
    for (const payload of invalidPayloads) {
      const storage = new MemoryStorage();
      storage.setItem(CAMPAIGN_MANIFEST_STORAGE_KEY, JSON.stringify(payload));
      expect(restoreCampaignManifest(storage)).toBe(false);
    }

    const oversized = new MemoryStorage();
    oversized.setItem(CAMPAIGN_MANIFEST_STORAGE_KEY, 'x'.repeat(256 * 1_024 + 1));
    expect(restoreCampaignManifest(oversized)).toBe(false);

    const overflow = storedCompletedCampaign();
    const manifest = JSON.parse(overflow.getItem(CAMPAIGN_MANIFEST_STORAGE_KEY) as string) as Record<string, unknown>;
    const campaign = campaignRecord(manifest);
    const snapshot = campaign.snapshot as Record<string, unknown>;
    const members = campaign.members as unknown[];
    const roster = snapshot.roster as unknown[];
    while (members.length < 6) members.push(structuredClone(members[0]));
    while (roster.length < 6) roster.push(structuredClone(roster[0]));
    overflow.setItem(CAMPAIGN_MANIFEST_STORAGE_KEY, JSON.stringify(manifest));
    expect(restoreCampaignManifest(overflow)).toBe(false);
  });
});

function completedCampaign(size: number): LabCampaignState {
  const snapshot = createRunCampaignSnapshot(demoTasks[0], createRoster(size));
  return {
    generation: 7,
    status: 'completed',
    startedAt: '2026-07-17T00:00:00.000Z',
    completedAt: '2026-07-17T00:10:00.000Z',
    snapshot,
    members: snapshot.roster.map((sortie, index) => ({
      rosterEntryId: sortie.rosterEntry.id,
      sortie,
      status: 'completed',
      stage: 'completed',
      jobId: `job-${index}`,
      runId: `run-${index}`,
      startedAt: `2026-07-17T00:00:0${index}.000Z`,
      completedAt: `2026-07-17T00:01:0${index}.000Z`,
      result: completedResult(index),
    })),
  };
}

function runningCampaign(): LabCampaignState {
  const snapshot = createRunCampaignSnapshot(demoTasks[0], createRoster(3));
  return {
    generation: 9,
    status: 'running',
    startedAt: '2026-07-17T00:00:00.000Z',
    snapshot,
    members: [
      {
        rosterEntryId: snapshot.roster[0].rosterEntry.id,
        sortie: snapshot.roster[0],
        status: 'completed',
        stage: 'completed',
        jobId: 'job-0',
        runId: 'run-0',
        startedAt: '2026-07-17T00:00:00.000Z',
        completedAt: '2026-07-17T00:01:00.000Z',
        result: completedResult(0),
      },
      {
        rosterEntryId: snapshot.roster[1].rosterEntry.id,
        sortie: snapshot.roster[1],
        status: 'running',
        stage: 'running',
        jobId: 'job-1',
        startedAt: '2026-07-17T00:02:00.000Z',
      },
      {
        rosterEntryId: snapshot.roster[2].rosterEntry.id,
        sortie: snapshot.roster[2],
        status: 'queued',
      },
    ],
  };
}

function createRoster(size: number): HangarRosterEntry[] {
  return Array.from({ length: size }, (_, index) => ({
    id: `roster-${index}`,
    airframeId: index % 2 === 0 ? 'j-35' : 'f-35',
    pilotId: index % 2 === 0 ? 'a3s' : 'codex',
    candidate: `./agents/candidate-${index}`,
    model: `provider/model-${index}`,
    effort: index % 2 === 0 ? 'high' : 'medium',
    callsign: `CALL-${index}`,
  }));
}

function completedResult(index: number): BenchRunResult {
  return {
    status: 'completed',
    run_id: `run-${index}`,
    task_id: demoTasks[0].id,
    score: `0.${index + 6}`,
    model_usage: {
      prompt_tokens: 100 + index,
      completion_tokens: 20 + index,
      total_tokens: 120 + index * 2,
      cache_read_tokens: 10,
      cache_write_tokens: 2,
      tool_calls_count: index + 1,
    },
  };
}

function completedResultWithNullableUsage(index: number): BenchRunResult {
  return {
    ...completedResult(index),
    model_usage: {
      prompt_tokens: 100 + index,
      completion_tokens: 20 + index,
      total_tokens: 120 + index * 2,
      cache_read_tokens: null,
      cache_write_tokens: null,
      tool_calls_count: null,
    },
  };
}

function storedCompletedCampaign(): MemoryStorage {
  const storage = new MemoryStorage();
  labState.campaign = completedCampaign(3);
  const stop = startCampaignManifestPersistence(storage, () => '2026-07-17T01:00:00.000Z');
  stop();
  return storage;
}

function campaignRecord(manifest: Record<string, unknown>): Record<string, unknown> {
  return manifest.campaign as Record<string, unknown>;
}
