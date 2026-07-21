import type { EffectiveSpreadsheetPageSetup } from '../work-spreadsheet-page-setup';

export function SpreadsheetHeaderFooterFields({
  pageSetup,
  onChange,
}: {
  pageSetup: EffectiveSpreadsheetPageSetup;
  onChange: (pageSetup: EffectiveSpreadsheetPageSetup) => void;
}) {
  const updateSection = (area: 'header' | 'footer', section: 'left' | 'center' | 'right', value: string) => {
    onChange({ ...pageSetup, [area]: { ...pageSetup[area], [section]: value } });
  };
  return (
    <fieldset className='work-spreadsheet-header-footer-fields'>
      <legend>页眉、页脚与页码</legend>
      <TemplateField
        label='页眉左侧'
        value={pageSetup.header.left}
        onChange={(value) => updateSection('header', 'left', value)}
      />
      <TemplateField
        label='页眉中间'
        value={pageSetup.header.center}
        onChange={(value) => updateSection('header', 'center', value)}
      />
      <TemplateField
        label='页眉右侧'
        value={pageSetup.header.right}
        onChange={(value) => updateSection('header', 'right', value)}
      />
      <TemplateField
        label='页脚左侧'
        value={pageSetup.footer.left}
        onChange={(value) => updateSection('footer', 'left', value)}
      />
      <TemplateField
        label='页脚中间'
        value={pageSetup.footer.center}
        onChange={(value) => updateSection('footer', 'center', value)}
      />
      <TemplateField
        label='页脚右侧'
        value={pageSetup.footer.right}
        onChange={(value) => updateSection('footer', 'right', value)}
      />
      <label>
        <span>起始页码</span>
        <input
          aria-label='起始页码'
          type='number'
          min={1}
          max={32767}
          value={pageSetup.pageNumberStart}
          onChange={(event) => onChange({ ...pageSetup, pageNumberStart: Number(event.target.value) })}
        />
      </label>
      <label>
        <span>打印页顺序</span>
        <select
          aria-label='打印页顺序'
          value={pageSetup.pageOrder}
          onChange={(event) =>
            onChange({
              ...pageSetup,
              pageOrder: event.target.value as EffectiveSpreadsheetPageSetup['pageOrder'],
            })
          }
        >
          <option value='overThenDown'>先向右，再向下</option>
          <option value='downThenOver'>先向下，再向右</option>
        </select>
      </label>
      <label className='toggle'>
        <input
          aria-label='页眉页脚随文档缩放'
          type='checkbox'
          checked={pageSetup.scaleWithDocument}
          onChange={(event) => onChange({ ...pageSetup, scaleWithDocument: event.target.checked })}
        />
        <span>页眉页脚随文档缩放</span>
      </label>
      <label className='toggle'>
        <input
          aria-label='页眉页脚与页边距对齐'
          type='checkbox'
          checked={pageSetup.alignWithMargins}
          onChange={(event) => onChange({ ...pageSetup, alignWithMargins: event.target.checked })}
        />
        <span>页眉页脚与页边距对齐</span>
      </label>
      <p className='tokens'>
        可用字段：{'{page}'}、{'{pages}'}、{'{sheet}'}、{'{file}'}、{'{path}'}、{'{date}'}、{'{time}'}
      </p>
    </fieldset>
  );
}

function TemplateField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label>
      <span>{label}</span>
      <input
        aria-label={label}
        value={value}
        placeholder={label.includes('右侧') ? 'Page {page} of {pages}' : ''}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
