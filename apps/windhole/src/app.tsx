import { useSnapshot } from 'valtio';
import { NoticeBanner } from './components/notice-banner';
import { TaskCatalog } from './components/task-catalog';
import { TelemetryPanel } from './components/telemetry-panel';
import { TopBar } from './components/top-bar';
import { WindTunnelScene } from './components/wind-tunnel-scene';
import { useBenchController } from './features/bench/use-bench-controller';
import { EngineeringWorkspace } from './features/engineering/components/engineering-workspace';
import { useEngineeringController } from './features/engineering/use-engineering-controller';
import { HangarScreen } from './features/hangar/components/hangar-screen';
import { activateHangarRosterEntry } from './features/hangar/hangar-roster-state';
import { ResultsWorkspace } from './features/results/components/results-workspace';
import { useResultController } from './features/results/use-result-controller';
import { labState } from './state/lab-state';

export function App() {
  const actions = useBenchController();
  const resultActions = useResultController();
  const engineeringActions = useEngineeringController();
  const state = useSnapshot(labState);

  return (
    <div className='app-shell'>
      <TopBar actions={actions} />
      {state.workspace === 'lab' ? (
        <main className='lab-grid'>
          <WindTunnelScene onActivateRosterEntry={activateHangarRosterEntry} />
          <TaskCatalog actions={actions} />
          <TelemetryPanel actions={actions} />
        </main>
      ) : null}
      {state.workspace === 'hangar' ? <HangarScreen /> : null}
      {state.workspace === 'results' ? <ResultsWorkspace actions={resultActions} /> : null}
      {state.workspace === 'engineering' ? <EngineeringWorkspace actions={engineeringActions} /> : null}

      {state.notice ? <NoticeBanner notice={state.notice} onDismiss={actions.dismissNotice} /> : null}
    </div>
  );
}
