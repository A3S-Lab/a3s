import {
  ArrowCounterClockwise,
  ArrowLeft,
  ArrowUUpLeft,
  ArrowUUpRight,
  ArrowsOut,
  CheckCircle,
  ClockCounterClockwise,
  DownloadSimple,
  FloppyDisk,
  Play,
  Plus,
  Stop,
  WarningCircle,
} from '@phosphor-icons/react';
import { A3SMark } from '@/components/home/a3s-mark';
import type { DebugTab } from './debug-panel';
import type { WorkflowPlaygroundCopy } from './workflow-copy';

interface WorkflowHeaderProps {
  copy: WorkflowPlaygroundCopy;
  lang: 'cn' | 'en';
  homeHref: string;
  languageHref: string;
  saveState: 'saved' | 'saving';
  canUndo: boolean;
  canRedo: boolean;
  running: boolean;
  issueCount: number;
  onUndo: () => void;
  onRedo: () => void;
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
  canUndo,
  canRedo,
  running,
  issueCount,
  onUndo,
  onRedo,
  onReset,
  onExport,
  onValidate,
  onRunToggle,
}: WorkflowHeaderProps) {
  return (
    <header className="a3s-workflow-header">
      <div className="a3s-workflow-header__identity">
        <a href={homeHref} aria-label={copy.backHome} title={copy.backHome}><ArrowLeft aria-hidden="true" /></a>
        <A3SMark aria-hidden="true" />
        <div><strong>{copy.workflowName}</strong><small><span>{copy.simulation}</span><i /><em>{saveState === 'saving' ? copy.saving : copy.saved}</em></small></div>
      </div>
      <div className="a3s-workflow-header__history">
        <button type="button" onClick={onUndo} disabled={!canUndo || running} aria-label={copy.undo} title={copy.undo}><ArrowUUpLeft aria-hidden="true" /></button>
        <button type="button" onClick={onRedo} disabled={!canRedo || running} aria-label={copy.redo} title={copy.redo}><ArrowUUpRight aria-hidden="true" /></button>
        <span aria-hidden="true" />
        <button type="button" onClick={onReset} disabled={running} aria-label={copy.reset} title={copy.reset}><ArrowCounterClockwise aria-hidden="true" /></button>
        <button type="button" onClick={onExport} disabled={running} aria-label={copy.exportGraph} title={copy.exportGraph}><DownloadSimple aria-hidden="true" /></button>
      </div>
      <div className="a3s-workflow-header__actions">
        <a href={languageHref} hrefLang={lang === 'cn' ? 'en' : 'zh-Hans'}>{lang === 'cn' ? 'EN' : '中文'}</a>
        <button type="button" onClick={onValidate} className={issueCount === 0 ? 'is-valid' : undefined}>
          {issueCount === 0 ? <CheckCircle aria-hidden="true" weight="fill" /> : <WarningCircle aria-hidden="true" />}
          <span>{copy.validate}</span>
        </button>
        <button className={running ? 'is-stop' : 'is-primary'} type="button" onClick={onRunToggle}>
          {running ? <Stop aria-hidden="true" weight="fill" /> : <Play aria-hidden="true" weight="fill" />}
          <span>{running ? copy.stop : copy.run}</span>
        </button>
      </div>
    </header>
  );
}

interface WorkflowRailProps {
  copy: WorkflowPlaygroundCopy;
  onAdd: () => void;
  onDebugTab: (tab: DebugTab) => void;
  onFitView: () => void;
}

export function WorkflowRail({ copy, onAdd, onDebugTab, onFitView }: WorkflowRailProps) {
  return (
    <nav className="a3s-workflow-rail" aria-label={copy.pageTitle}>
      <button className="is-primary" type="button" onClick={onAdd} aria-label={copy.addNode} title={copy.addNode}><Plus aria-hidden="true" weight="bold" /></button>
      <button type="button" onClick={() => onDebugTab('variables')} aria-label={copy.variables} title={copy.variables}><FloppyDisk aria-hidden="true" /></button>
      <button type="button" onClick={() => onDebugTab('history')} aria-label={copy.history} title={copy.history}><ClockCounterClockwise aria-hidden="true" /></button>
      <span />
      <button className="a3s-workflow-rail__fit" type="button" onClick={onFitView} aria-label={copy.fitView} title={copy.fitView}><ArrowsOut aria-hidden="true" /><span>{copy.fitView}</span></button>
    </nav>
  );
}
