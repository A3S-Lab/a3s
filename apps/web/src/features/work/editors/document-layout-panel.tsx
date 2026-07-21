import { clampDocumentMargin } from '../work-document-layout';
import { documentPageChromeLegacyFields, normalizeDocumentPageChrome } from '../work-document-page-chrome';
import type { WorkDocumentMargins, WorkDocumentSectionLayout } from '../work-types';
import { DocumentColumnsPanel } from './document-columns-panel';
import { DocumentPageChromePanel } from './document-page-chrome-panel';

export function DocumentLayoutPanel({
  layout,
  sectionIndex,
  sectionCount,
  onChange,
  onInsertSection,
  onMergeSection,
}: {
  layout: WorkDocumentSectionLayout;
  sectionIndex: number;
  sectionCount: number;
  onChange: (layout: WorkDocumentSectionLayout) => void;
  onInsertSection: () => void;
  onMergeSection: () => void;
}) {
  const marginFields: Array<[keyof WorkDocumentMargins, string]> = [
    ['top', '上'],
    ['right', '右'],
    ['bottom', '下'],
    ['left', '左'],
  ];
  const update = (patch: Partial<WorkDocumentSectionLayout>) => onChange({ ...layout, ...patch });
  return (
    <fieldset className='work-document-layout-panel' aria-label='页面设置'>
      <legend>
        第 {sectionIndex + 1} 节 · 共 {sectionCount} 节
      </legend>
      <label>
        <span>本节之后</span>
        <select
          aria-label='分节方式'
          value={layout.breakAfter}
          onChange={(event) => update({ breakAfter: event.target.value as WorkDocumentSectionLayout['breakAfter'] })}
        >
          <option value='nextPage'>下一页</option>
          <option value='continuous'>连续</option>
          <option value='evenPage'>下一偶数页</option>
          <option value='oddPage'>下一奇数页</option>
          <option value='nextColumn'>下一栏（预览按连续节）</option>
        </select>
      </label>
      <div className='work-document-section-actions'>
        <button type='button' onClick={onInsertSection}>
          插入新节
        </button>
        <button type='button' disabled={sectionIndex === 0} onClick={onMergeSection}>
          与上一节合并
        </button>
      </div>
      <label>
        <span>纸张</span>
        <select
          aria-label='纸张大小'
          value={layout.pageSize}
          onChange={(event) => update({ pageSize: event.target.value as WorkDocumentSectionLayout['pageSize'] })}
        >
          <option value='a4'>A4</option>
          <option value='letter'>Letter</option>
        </select>
      </label>
      <label>
        <span>方向</span>
        <select
          aria-label='页面方向'
          value={layout.orientation}
          onChange={(event) => update({ orientation: event.target.value as WorkDocumentSectionLayout['orientation'] })}
        >
          <option value='portrait'>纵向</option>
          <option value='landscape'>横向</option>
        </select>
      </label>
      <DocumentColumnsPanel columns={layout.columns} onChange={(columns) => update({ columns })} />
      <fieldset>
        <legend>页边距（毫米）</legend>
        {marginFields.map(([side, label]) => (
          <label key={side}>
            <span>{label}</span>
            <input
              type='number'
              min='5'
              max='60'
              step='1'
              aria-label={`${label}页边距`}
              value={layout.margins[side]}
              onChange={(event) =>
                update({
                  margins: {
                    ...layout.margins,
                    [side]: clampDocumentMargin(Number(event.target.value)),
                  },
                })
              }
            />
          </label>
        ))}
      </fieldset>
      <DocumentPageChromePanel
        pageChrome={normalizeDocumentPageChrome(layout.pageChrome, layout)}
        onChange={(pageChrome) => update({ pageChrome, ...documentPageChromeLegacyFields(pageChrome) })}
      />
      <label className='work-document-page-number-option'>
        <span>本节页码从</span>
        <input
          type='number'
          min='1'
          max='9999'
          aria-label='起始页码'
          value={Math.max(1, layout.pageNumberStart ?? 1)}
          onChange={(event) =>
            update({
              pageNumberStart: Math.min(9999, Math.max(1, Math.round(Number(event.target.value) || 1))),
            })
          }
        />
        <span>开始</span>
      </label>
    </fieldset>
  );
}
