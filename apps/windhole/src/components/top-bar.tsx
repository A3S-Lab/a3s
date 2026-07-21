import { Activity, Map as MapIcon, Radio, RefreshCw, Trophy, Warehouse, Wrench } from 'lucide-react';
import { useSnapshot } from 'valtio';
import type { BenchController } from '../features/bench/use-bench-controller';
import { labState } from '../state/lab-state';

interface TopBarProps {
  actions: BenchController;
}

export function TopBar({ actions }: TopBarProps) {
  const state = useSnapshot(labState);
  const mode = state.connection.mode;

  return (
    <header className='top-bar'>
      <div className='brand-lockup'>
        <span className='brand-mark' aria-hidden='true'>
          <span />
          <span />
          <span />
        </span>
        <strong className='brand-title'>A3S智能体评测</strong>
      </div>

      <nav className='workspace-nav' aria-label='A3S智能体评测工作区'>
        <button
          className={state.workspace === 'lab' ? 'is-active' : ''}
          onClick={() => {
            labState.workspace = 'lab';
          }}
          aria-current={state.workspace === 'lab' ? 'page' : undefined}
        >
          <MapIcon size={14} aria-hidden='true' />
          <span>作战地图</span>
        </button>
        <button
          className={state.workspace === 'hangar' ? 'is-active' : ''}
          onClick={() => {
            labState.workspace = 'hangar';
          }}
          aria-current={state.workspace === 'hangar' ? 'page' : undefined}
        >
          <Warehouse size={14} aria-hidden='true' />
          <span>机库编队</span>
        </button>
        <button
          className={state.workspace === 'results' ? 'is-active' : ''}
          onClick={() => {
            labState.workspace = 'results';
          }}
          aria-current={state.workspace === 'results' ? 'page' : undefined}
        >
          <Trophy size={14} aria-hidden='true' />
          <span>战报</span>
        </button>
        <button
          className={state.workspace === 'engineering' ? 'is-active' : ''}
          onClick={() => {
            labState.workspace = 'engineering';
          }}
          aria-current={state.workspace === 'engineering' ? 'page' : undefined}
        >
          <Wrench size={14} aria-hidden='true' />
          <span>工程舱</span>
        </button>
      </nav>

      <div className='top-spacer' />

      <output className='top-readout' aria-label='试验数据状态'>
        <Activity size={14} aria-hidden='true' />
        <span>REALTIME</span>
        <span className='readout-separator'>/</span>
        <span>THREE.JS</span>
      </output>

      <button
        className={`connection-pill connection-${mode}`}
        onClick={() => void actions.refresh()}
        title={state.connection.message}
        aria-label={`${state.connection.message}，点击重新检测`}
      >
        <Radio size={13} aria-hidden='true' />
        <span>{mode === 'live' ? 'CLI LIVE' : mode === 'checking' ? 'CHECKING' : 'PREVIEW'}</span>
        <RefreshCw className={mode === 'checking' ? 'spin' : ''} size={12} aria-hidden='true' />
      </button>
    </header>
  );
}
