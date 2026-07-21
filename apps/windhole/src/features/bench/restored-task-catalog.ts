import { labState, type RunSortieTaskSnapshot } from '../../state/lab-state';
import type { BenchTask } from '../../types/bench';
import { RESTORED_TASK_MISSING_REASON } from './task-deployment-status';

interface RestoredTaskCandidate {
  readonly startedAt: string;
  readonly task: RunSortieTaskSnapshot;
}

export function reconcileRestoredTaskCatalog(): void {
  const candidates = restoredTaskCandidates();
  for (const candidate of candidates) ensureCatalogTask(labState.catalog.tasks, candidate.task, false);

  const latest = latestCandidate(candidates);
  if (latest) labState.catalog.selectedTaskId = latest.task.id;
}

export function replaceTaskCatalog(tasks: readonly BenchTask[]): void {
  const selectedTaskId = labState.catalog.selectedTaskId;
  const nextTasks = [...tasks];
  for (const candidate of restoredTaskCandidates()) ensureCatalogTask(nextTasks, candidate.task, true);

  labState.catalog.tasks = nextTasks;
  if (!nextTasks.some((task) => task.id === selectedTaskId)) {
    labState.catalog.selectedTaskId = nextTasks[0]?.id ?? '';
  }
}

function restoredTaskCandidates(): RestoredTaskCandidate[] {
  const candidates: RestoredTaskCandidate[] = [];
  if (labState.run.stage !== 'idle' && labState.run.startedAt && labState.run.sortie) {
    candidates.push({ startedAt: labState.run.startedAt, task: labState.run.sortie.task });
  }
  if (labState.campaign.status !== 'idle' && labState.campaign.startedAt && labState.campaign.snapshot) {
    candidates.push({ startedAt: labState.campaign.startedAt, task: labState.campaign.snapshot.task });
  }
  return candidates;
}

function latestCandidate(candidates: readonly RestoredTaskCandidate[]): RestoredTaskCandidate | undefined {
  return candidates.reduce<RestoredTaskCandidate | undefined>((latest, candidate) => {
    if (!latest || Date.parse(candidate.startedAt) > Date.parse(latest.startedAt)) return candidate;
    return latest;
  }, undefined);
}

function ensureCatalogTask(catalog: BenchTask[], task: RunSortieTaskSnapshot, preservedAfterRefresh: boolean): void {
  if (catalog.some((candidate) => candidate.id === task.id)) return;
  catalog.push(cloneTask(task, preservedAfterRefresh));
}

function cloneTask(task: RunSortieTaskSnapshot, preservedAfterRefresh: boolean): BenchTask {
  return {
    id: task.id,
    path: task.path,
    name: task.name,
    category: task.category,
    execution_class: task.execution_class,
    availability: preservedAfterRefresh ? 'blocked' : task.availability,
    availability_reason: preservedAfterRefresh ? RESTORED_TASK_MISSING_REASON : task.availability_reason,
    admission: task.admission,
    admission_reason: task.admission_reason,
    provenance_ref: task.provenance_ref,
    description: task.description,
    tags: task.tags ? [...task.tags] : undefined,
  };
}
