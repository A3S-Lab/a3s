import { AlertTriangle, CheckCircle2, Info, ShieldAlert } from 'lucide-react';
import { Button, Dialog } from '../../../design-system/primitives';
import type { WorkCompatibilityReport } from '../work-types';

export function WorkCompatibilityDialog({
  report,
  mode,
  busy = false,
  onClose,
  onConfirm,
}: {
  report: WorkCompatibilityReport;
  mode: 'import' | 'export' | 'save' | 'review';
  busy?: boolean;
  onClose: () => void;
  onConfirm?: () => void;
}) {
  const warnings = report.issues.filter((issue) => issue.severity === 'warning').length;
  const errors = report.issues.filter((issue) => issue.severity === 'error').length;
  const title =
    mode === 'import'
      ? '导入前检查'
      : mode === 'export'
        ? '导出前兼容性检查'
        : mode === 'save'
          ? '保存前兼容性检查'
          : '文件兼容性';
  const description =
    mode === 'import'
      ? `A3S Work 已分析 ${report.sourceName}，请在保存转换结果前检查兼容性。`
      : mode === 'export'
        ? '这些项目可能与原始 Office 文件不同。原始文件仍可单独下载。'
        : mode === 'save'
          ? '写入本地 Office 文件后，这些项目可能与原始版本不同。'
          : `来自 ${report.sourceFormat} 的转换报告。`;

  return (
    <Dialog
      title={title}
      description={description}
      onClose={onClose}
      closeDisabled={busy}
      footer={
        mode === 'review' ? undefined : (
          <>
            <Button tone='quiet' disabled={busy} onClick={onClose}>
              取消
            </Button>
            <Button tone='primary' loading={busy} data-autofocus onClick={onConfirm}>
              {mode === 'import' ? '继续导入并保留原文件' : mode === 'save' ? '仍然保存' : '仍然导出'}
            </Button>
          </>
        )
      }
    >
      <section className='work-compatibility-report'>
        <header className={errors ? 'error' : warnings ? 'warning' : 'clear'}>
          <span>
            {errors ? <ShieldAlert size={19} /> : warnings ? <AlertTriangle size={19} /> : <CheckCircle2 size={19} />}
          </span>
          <div>
            <strong>
              {errors
                ? `${errors} 个项目无法转换`
                : warnings
                  ? `${warnings} 个项目可能发生变化`
                  : '没有发现已知的兼容性问题'}
            </strong>
            <small>原始 {report.sourceFormat} 文件会与可编辑副本一起保留，可随时下载。</small>
          </div>
        </header>
        {report.issues.length > 0 && (
          <ol>
            {report.issues.map((issue, index) => (
              <li className={issue.severity} key={`${issue.code}-${issue.location ?? ''}-${index}`}>
                <span>{issue.severity === 'info' ? <Info size={14} /> : <AlertTriangle size={14} />}</span>
                <div>
                  <strong>{issue.feature}</strong>
                  <p>{issue.message}</p>
                  {issue.location && <small>{issue.location}</small>}
                </div>
              </li>
            ))}
          </ol>
        )}
      </section>
    </Dialog>
  );
}
