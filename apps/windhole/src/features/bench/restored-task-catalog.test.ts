import { beforeEach, describe, expect, it } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { createRunCampaignSnapshot, createRunSortieSnapshot, labState } from '../../state/lab-state';
import type { BenchTask } from '../../types/bench';
import { DEFAULT_HANGAR_ROSTER } from '../hangar/hangar-configuration';
import { restoreCampaignManifest, startCampaignManifestPersistence } from './campaign-manifest-store';
import { replaceTaskCatalog } from './restored-task-catalog';
import { restoreSingleRunManifest, startSingleRunManifestPersistence } from './single-run-manifest-store';
import { RESTORED_TASK_MISSING_REASON, taskDeploymentStatus } from './task-deployment-status';

class MemoryStorage {
  readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

const singleTask = restoredTask('private_single_task', 'Private Single Task');
const campaignTask = restoredTask('private_campaign_task', 'Private Campaign Task');

beforeEach(() => {
  labState.catalog = {
    tasks: demoTasks.map((task) => ({ ...task, tags: task.tags ? [...task.tags] : undefined })),
    selectedTaskId: demoTasks[0].id,
    query: '',
    category: 'all',
    includeBlocked: false,
  };
  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
});

describe('restored task catalog', () => {
  it.each([
    {
      name: 'keeps a newer campaign selected after the older single manifest restores second',
      singleStartedAt: '2026-07-17T00:00:00.000Z',
      singleCompletedAt: '2026-07-17T00:01:00.000Z',
      campaignStartedAt: '2026-07-17T00:10:00.000Z',
      campaignCompletedAt: '2026-07-17T00:11:00.000Z',
      selectedTaskId: campaignTask.id,
    },
    {
      name: 'selects a newer single run instead of the older campaign restored first',
      singleStartedAt: '2026-07-17T00:10:00.000Z',
      singleCompletedAt: '2026-07-17T00:11:00.000Z',
      campaignStartedAt: '2026-07-17T00:00:00.000Z',
      campaignCompletedAt: '2026-07-17T00:01:00.000Z',
      selectedTaskId: singleTask.id,
    },
  ])('$name', (scenario) => {
    const storage = storedEvaluations(scenario);

    expect(restoreCampaignManifest(storage)).toBe(true);
    expect(restoreSingleRunManifest(storage)).toBe(true);

    expect(labState.catalog.selectedTaskId).toBe(scenario.selectedTaskId);
    expect(labState.catalog.tasks.map((task) => task.id)).toEqual(
      expect.arrayContaining([singleTask.id, campaignTask.id])
    );

    const restoredSingleTask = labState.catalog.tasks.find((task) => task.id === singleTask.id);
    expect(restoredSingleTask).toEqual(singleTask);
    expect(restoredSingleTask).not.toBe(labState.run.sortie?.task);
    expect(restoredSingleTask?.tags).not.toBe(labState.run.sortie?.task.tags);
  });

  it('keeps missing restored maps visible but blocked after catalog refresh, then trusts an authoritative match', () => {
    const storage = storedEvaluations({
      singleStartedAt: '2026-07-17T00:10:00.000Z',
      singleCompletedAt: '2026-07-17T00:11:00.000Z',
      campaignStartedAt: '2026-07-17T00:00:00.000Z',
      campaignCompletedAt: '2026-07-17T00:01:00.000Z',
    });
    expect(restoreCampaignManifest(storage)).toBe(true);
    expect(restoreSingleRunManifest(storage)).toBe(true);

    replaceTaskCatalog([{ ...demoTasks[0] }]);

    const preservedTask = labState.catalog.tasks.find((task) => task.id === singleTask.id);
    expect(labState.catalog.selectedTaskId).toBe(singleTask.id);
    expect(preservedTask).toMatchObject({
      availability: 'blocked',
      availability_reason: RESTORED_TASK_MISSING_REASON,
    });
    expect(taskDeploymentStatus(preservedTask as BenchTask, undefined)).toMatchObject({
      deployable: false,
      message: expect.stringContaining('仅可用于场景与归属展示'),
    });

    replaceTaskCatalog([{ ...singleTask, name: 'Authoritative Single Task' }]);

    const authoritativeTask = labState.catalog.tasks.find((task) => task.id === singleTask.id);
    expect(authoritativeTask).toMatchObject({
      name: 'Authoritative Single Task',
      availability: 'ready',
      availability_reason: singleTask.availability_reason,
    });
    expect(taskDeploymentStatus(authoritativeTask as BenchTask, undefined).deployable).toBe(true);
  });
});

interface StoredEvaluationTimes {
  readonly singleStartedAt: string;
  readonly singleCompletedAt: string;
  readonly campaignStartedAt: string;
  readonly campaignCompletedAt: string;
}

function storedEvaluations(times: StoredEvaluationTimes): MemoryStorage {
  const storage = new MemoryStorage();
  const rosterEntry = DEFAULT_HANGAR_ROSTER[0];
  const campaignSnapshot = createRunCampaignSnapshot(campaignTask, [rosterEntry]);
  labState.campaign = {
    generation: 1,
    status: 'completed',
    startedAt: times.campaignStartedAt,
    completedAt: times.campaignCompletedAt,
    snapshot: campaignSnapshot,
    members: [
      {
        rosterEntryId: rosterEntry.id,
        sortie: campaignSnapshot.roster[0],
        status: 'completed',
        stage: 'completed',
        jobId: 'campaign-job',
        runId: 'campaign-run',
        startedAt: times.campaignStartedAt,
        completedAt: times.campaignCompletedAt,
        result: {
          status: 'completed',
          run_id: 'campaign-run',
          task_id: campaignTask.id,
        },
      },
    ],
  };
  const stopCampaign = startCampaignManifestPersistence(storage, () => '2026-07-17T01:00:00.000Z');
  stopCampaign();

  const singleSortie = createRunSortieSnapshot(singleTask, rosterEntry, {
    task: singleTask.id,
    candidate: rosterEntry.candidate,
    model: rosterEntry.model,
    locked: false,
  });
  labState.run = {
    mode: 'live',
    stage: 'completed',
    jobId: 'single-job',
    runId: 'single-run',
    startedAt: times.singleStartedAt,
    completedAt: times.singleCompletedAt,
    sortie: singleSortie,
    result: {
      status: 'completed',
      run_id: 'single-run',
      task_id: singleTask.id,
    },
  };
  const stopSingle = startSingleRunManifestPersistence(storage, () => '2026-07-17T01:00:00.000Z');
  stopSingle();

  labState.run = { stage: 'idle' };
  labState.campaign = { generation: 0, status: 'idle', members: [] };
  return storage;
}

function restoredTask(id: string, name: string): BenchTask {
  return {
    ...demoTasks[0],
    id,
    path: `private/${id}`,
    name,
    provenance_ref: `restored/${id}`,
    tags: ['restored', id],
  };
}
