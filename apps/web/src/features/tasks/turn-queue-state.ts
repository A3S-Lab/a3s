import { appState } from '../../state/app-state';
import type { TurnQueue } from '../../types/api';

export function applyTurnQueueSnapshot(queue: TurnQueue): void {
  appState.turnQueues[queue.sessionId] = queue;
}
