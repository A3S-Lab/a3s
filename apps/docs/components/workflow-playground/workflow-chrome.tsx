import {
  ArrowCounterClockwise,
  ArrowLeft,
  ArrowUUpLeft,
  ArrowUUpRight,
  CheckCircle,
  ClockCounterClockwise,
  Cursor,
  Database,
  DotsThree,
  DownloadSimple,
  Hand,
  Play,
  Plus,
  Stop,
  WarningCircle,
} from '@phosphor-icons/react';
import { A3SMark } from '@/components/home/a3s-mark';
import type { DebugTab } from './debug-panel';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import type { WorkflowCanvasMode } from './workflow-model';

interface WorkflowHeaderProps {
  copy: WorkflowPlaygroundCopy;
  lang: 'cn' | 'en';
  homeHref: string;
  languageHref: string;
  saveState: 'saved' | 'saving';
  running: boolean;
  issueCount: number;
  onReset: () => void;
  onExport: () => void;
  onValidate: () => void;
  onRunToggle: () => void;
}

export function WorkflowHeader({
  copy,
  lang,
  homeHref,
  languageHref,
  saveState,
  running,
  issueCount,
  onReset,
  onExport,
  onValidate,
  onRunToggle,
}: WorkflowHeaderProps) {
  return (
    <header className="a3s-workflow-header">
      <div className="a3s-workflow-header__identity">
        <a href={homeHref} aria-label={copy.backHome} title={copy.backHome}>
          <ArrowLeft aria-hidden="true" />
        </a>
        <A3SMark aria-hidden="true" />
        <div>
          <strong>{copy.workflowName}</strong>
          <small>
            <span>{copy.simulation}</span>
            <i />
            <em>{saveState === 'saving' ? copy.saving : copy.saved}</em>
          </small>
        </div>
      </div>
      <div className="a3s-workflow-header__actions">
        <a href={languageHref} hrefLang={lang === 'cn' ? 'en' : 'zh-Hans'}>
          {lang === 'cn' ? 'EN' : '中文'}
        </a>
        <button
          type="button"
          onClick={onValidate}
          className={`is-validate${issueCount === 0 ? ' is-valid' : ''}`}
        >
          {issueCount === 0
            ? <CheckCircle aria-hidden="true" weight="fill" />
            : <WarningCircle aria-hidden="true" />}
          <span>{copy.validate}</span>
        </button>
        <button className={running ? 'is-stop' : 'is-primary'} type="button" onClick={onRunToggle}>
          {running
            ? <Stop aria-hidden="true" weight="fill" />
            : <Play aria-hidden="true" weight="fill" />}
          <span>{running ? copy.stop : copy.run}</span>
        </button>
        <details className="a3s-workflow-header__menu">
          <summary aria-label={copy.moreActions} title={copy.moreActions}>
            <DotsThree aria-hidden="true" weight="bold" />
          </summary>
          <div>
            <button type="button" onClick={onReset} disabled={running}>
              <ArrowCounterClockwise aria-hidden="true" />
              <span>{copy.reset}</span>
            </button>
            <button type="button" onClick={onExport} disabled={running}>
              <DownloadSimple aria-hidden="true" />
              <span>{copy.exportGraph}</span>
            </button>
          </div>
        </details>
      </div>
    </header>
  );
}

interface WorkflowRailProps {
  copy: WorkflowPlaygroundCopy;
  mode: WorkflowCanvasMode;
  onAdd: () => void;
  onModeChange: (mode: WorkflowCanvasMode) => void;
}

export function WorkflowRail({ copy, mode, onAdd, onModeChange }: WorkflowRailProps) {
  return (
    <nav className="a3s-workflow-rail" aria-label={copy.pageTitle}>
      <button
        className="is-primary"
        type="button"
        onClick={onAdd}
        aria-label={copy.addNode}
        title={copy.addNode}
      >
        <Plus aria-hidden="true" weight="bold" />
      </button>
      <button
        className={mode === 'select' ? 'is-active' : undefined}
        type="button"
        onClick={() => onModeChange('select')}
        aria-pressed={mode === 'select'}
        aria-label={copy.selectMode}
        title={copy.selectMode}
      >
        <Cursor aria-hidden="true" />
      </button>
      <button
        className={mode === 'pan' ? 'is-active' : undefined}
        type="button"
        onClick={() => onModeChange('pan')}
        aria-pressed={mode === 'pan'}
        aria-label={copy.panMode}
        title={copy.panMode}
      >
        <Hand aria-hidden="true" />
      </button>
    </nav>
  );
}

interface WorkflowCanvasDockProps {
  copy: WorkflowPlaygroundCopy;
  canUndo: boolean;
  canRedo: boolean;
  running: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onDebugTab: (tab: DebugTab) => void;
}

export function WorkflowCanvasDock({
  copy,
  canUndo,
  canRedo,
  running,
  onUndo,
  onRedo,
  onDebugTab,
}: WorkflowCanvasDockProps) {
  return (
    <>
      <nav className="a3s-workflow-canvas-tools" aria-label={copy.canvasTools}>
        <button
          type="button"
          onClick={onUndo}
          disabled={!canUndo || running}
          aria-label={copy.undo}
          title={copy.undo}
        >
          <ArrowUUpLeft aria-hidden="true" />
        </button>
        <button
          type="button"
          onClick={onRedo}
          disabled={!canRedo || running}
          aria-label={copy.redo}
          title={copy.redo}
        >
          <ArrowUUpRight aria-hidden="true" />
        </button>
        <span aria-hidden="true" />
        <button
          type="button"
          onClick={() => onDebugTab('history')}
          aria-label={copy.history}
          title={copy.history}
        >
          <ClockCounterClockwise aria-hidden="true" />
        </button>
      </nav>
      <button
        className="a3s-workflow-cached-variables"
        type="button"
        onClick={() => onDebugTab('variables')}
      >
        <Database aria-hidden="true" />
        <span>{copy.viewCachedVariables}</span>
      </button>
    </>
  );
}
