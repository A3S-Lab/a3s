import { useCallback, useState } from 'react';
import { useSnapshot } from 'valtio';
import { isEvaluationActive, labState } from '../../../state/lab-state';
import { hangarDraftMatchesRosterEntry } from '../hangar-roster-state';
import { useHangarController } from '../use-hangar-controller';
import type { HangarPreviewStatus } from './hangar-preview';
import { HangarScenePreview } from './hangar-scene-preview';
import { HangarWorkspace } from './hangar-workspace';

export function HangarScreen() {
  const state = useSnapshot(labState);
  const actions = useHangarController();
  const [previewStatus, setPreviewStatus] = useState<HangarPreviewStatus>('loading');
  const [resetVersion, setResetVersion] = useState(0);
  const handleStatusChange = useCallback((status: HangarPreviewStatus) => setPreviewStatus(status), []);
  const draft = { ...state.hangar.draft };
  const activeEntry = state.hangar.roster.find((entry) => entry.id === state.hangar.activeEntryId);
  const deployReady = hangarDraftMatchesRosterEntry(draft, activeEntry);
  const runActive = isEvaluationActive(state.run.stage, state.campaign.status);

  return (
    <HangarWorkspace
      draft={draft}
      roster={state.hangar.roster}
      selectedRosterId={state.hangar.activeEntryId}
      previewStatus={previewStatus}
      preview={<HangarScenePreview draft={draft} resetVersion={resetVersion} onStatusChange={handleStatusChange} />}
      onDraftChange={actions.replaceDraft}
      onAddToRoster={(nextDraft) => {
        actions.replaceDraft(nextDraft);
        actions.addToRoster();
      }}
      onSelectRoster={(entryId) => actions.activateRosterEntry(entryId)}
      onRemoveFromRoster={actions.removeRosterEntry}
      onUpdateRoster={() => actions.updateRosterEntry(state.hangar.activeEntryId)}
      onResetPreview={() => setResetVersion((version) => version + 1)}
      onDeploy={() => actions.activateRosterEntry(state.hangar.activeEntryId, true)}
      deployReady={deployReady}
      updateReady={!deployReady}
      disabled={runActive}
    />
  );
}
