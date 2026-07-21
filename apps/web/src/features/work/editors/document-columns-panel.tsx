import {
  normalizeDocumentColumns,
  setCustomDocumentColumns,
  updateDocumentColumnWidth,
} from '../work-document-columns';
import type { WorkDocumentColumns } from '../work-types';

export function DocumentColumnsPanel({
  columns,
  onChange,
}: {
  columns: WorkDocumentColumns;
  onChange: (columns: WorkDocumentColumns) => void;
}) {
  const normalized = normalizeDocumentColumns(columns);
  const custom = normalized.custom;
  const updateCustomSpacing = (index: number, spacing: number) => {
    if (!custom) return;
    onChange(
      normalizeDocumentColumns({
        ...normalized,
        custom: custom.map((column, columnIndex) => (columnIndex === index ? { ...column, spacing } : column)),
      })
    );
  };

  return (
    <fieldset className='work-document-columns-panel'>
      <legend>分栏</legend>
      <label>
        <span>栏数</span>
        <input
          type='number'
          min='1'
          max='6'
          step='1'
          aria-label='分栏数量'
          value={normalized.count}
          onChange={(event) =>
            onChange(
              normalizeDocumentColumns({
                ...normalized,
                count: Number(event.target.value),
              })
            )
          }
        />
      </label>
      {!normalized.custom && (
        <label>
          <span>间距</span>
          <input
            type='number'
            min='0'
            max='30'
            step='0.5'
            aria-label='分栏间距'
            value={normalized.spacing}
            onChange={(event) =>
              onChange(
                normalizeDocumentColumns({
                  ...normalized,
                  spacing: Number(event.target.value),
                })
              )
            }
          />
        </label>
      )}
      <label className='work-document-column-option'>
        <input
          type='checkbox'
          aria-label='自定义栏宽'
          disabled={normalized.count < 2}
          checked={Boolean(normalized.custom)}
          onChange={(event) => onChange(setCustomDocumentColumns(normalized, event.target.checked))}
        />
        <span>自定义栏宽</span>
      </label>
      <label className='work-document-column-option'>
        <input
          type='checkbox'
          aria-label='分栏分隔线'
          checked={normalized.separator}
          onChange={(event) => onChange({ ...normalized, separator: event.target.checked })}
        />
        <span>分隔线</span>
      </label>
      {custom && (
        <div className='work-document-custom-columns'>
          {custom.map((column, index) => (
            <div key={`document-column-${index + 1}`} className='work-document-custom-column'>
              <strong>第 {index + 1} 栏</strong>
              <label>
                <span>宽度 %</span>
                <input
                  type='number'
                  min='5'
                  max={100 - (normalized.count - 1) * 5}
                  step='0.5'
                  aria-label={`第 ${index + 1} 栏宽度百分比`}
                  value={column.widthPercent}
                  onChange={(event) =>
                    onChange(updateDocumentColumnWidth(normalized, index, Number(event.target.value)))
                  }
                />
              </label>
              {index < custom.length - 1 && (
                <label>
                  <span>栏后间距</span>
                  <input
                    type='number'
                    min='0'
                    max='30'
                    step='0.5'
                    aria-label={`第 ${index + 1} 栏后间距`}
                    value={column.spacing}
                    onChange={(event) => updateCustomSpacing(index, Number(event.target.value))}
                  />
                </label>
              )}
            </div>
          ))}
          <p>编辑区保持连续多栏输入；预览、PDF 与 DOCX 使用这里的栏宽比例和独立间距。</p>
        </div>
      )}
    </fieldset>
  );
}
