import { AlertTriangle, RefreshCw, Sparkles } from 'lucide-react';
import { Button } from '../../../design-system/primitives';
import type { EvolutionCandidate, EvolutionCandidateState, EvolutionKind } from '../../../types/api';
import { evolutionKindLabel, evolutionStateLabel } from '../evolution-format';

export function CandidateButton({
  candidate,
  selected,
  busy,
  onSelect,
}: {
  candidate: EvolutionCandidate;
  selected: boolean;
  busy: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type='button'
      className={`evolution-candidate${selected ? ' selected' : ''}`}
      aria-current={selected ? 'true' : undefined}
      aria-busy={busy || undefined}
      onClick={onSelect}
    >
      <span className='evolution-candidate-heading'>
        <KindBadge kind={candidate.kind} />
        <StateBadge state={candidate.state} />
        {candidate.updateAvailable && <i>有更新</i>}
      </span>
      <strong>{candidate.title}</strong>
    </button>
  );
}

export function EvolutionLoadingState() {
  return (
    <output className='evolution-loading' aria-label='正在加载学习内容'>
      <Sparkles size={22} />
      <span>正在加载…</span>
    </output>
  );
}

export function EvolutionErrorState({ error, onRetry }: { error: string | null; onRetry: () => void }) {
  return (
    <div className='memory-state-card' role='alert'>
      <span>
        <AlertTriangle size={22} />
      </span>
      <h2>无法加载学习内容</h2>
      <p title={error || undefined}>暂时无法读取，请稍后重试。</p>
      <Button onClick={onRetry}>
        <RefreshCw size={14} />
        重新加载
      </Button>
    </div>
  );
}

export function EvolutionEmptyState() {
  return (
    <div className='memory-state-card evolution-empty'>
      <span>
        <Sparkles size={24} />
      </span>
      <h2>还没有可确认的内容</h2>
      <p>完成更多任务后，A3S 会把稳定的偏好和做法列在这里。</p>
    </div>
  );
}

export function KindBadge({ kind }: { kind: EvolutionKind }) {
  return (
    <span className='evolution-kind-badge' data-kind={kind}>
      {evolutionKindLabel(kind)}
    </span>
  );
}

export function StateBadge({ state }: { state: EvolutionCandidateState }) {
  return (
    <span className='evolution-state-badge' data-state={state}>
      {evolutionStateLabel(state)}
    </span>
  );
}
