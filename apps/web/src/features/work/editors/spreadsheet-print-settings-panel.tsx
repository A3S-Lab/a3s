import { useEffect, useState } from 'react';
import {
  formatSpreadsheetColumnPageBreaks,
  formatSpreadsheetRowPageBreaks,
  parseSpreadsheetColumnPageBreaks,
  parseSpreadsheetRowPageBreaks,
} from '../work-spreadsheet-page-breaks';
import {
  normalizeSpreadsheetPrintArea,
  normalizeSpreadsheetPrintTitleColumns,
  normalizeSpreadsheetPrintTitleRows,
} from '../work-spreadsheet-ranges';
import { effectiveSpreadsheetPageSetup, type EffectiveSpreadsheetPageSetup } from '../work-spreadsheet-page-setup';
import type { WorkSpreadsheetContent } from '../work-types';
import { SpreadsheetHeaderFooterFields } from './spreadsheet-header-footer-fields';

export function spreadsheetPrintSettingCount(content: WorkSpreadsheetContent): number {
  return new Set([
    ...(content.printAreas ?? []).map((area) => area.sheetId),
    ...(content.printTitles ?? []).map((titles) => titles.sheetId),
    ...(content.pageBreaks ?? []).map((pageBreaks) => pageBreaks.sheetId),
    ...(content.pageSetups ?? []).map((pageSetup) => pageSetup.sheetId),
  ]).size;
}

export function SpreadsheetPrintSettingsPanel({
  content,
  onChange,
}: {
  content: WorkSpreadsheetContent;
  onChange: (content: WorkSpreadsheetContent) => void;
}) {
  const availableSheets = content.sheets.filter((sheet): sheet is typeof sheet & { id: string } => Boolean(sheet.id));
  const initialSheet = availableSheets.find((sheet) => sheet.status === 1)?.id ?? availableSheets[0]?.id ?? '';
  const [sheetId, setSheetId] = useState(initialSheet);
  const savedArea = content.printAreas?.find((area) => area.sheetId === sheetId);
  const savedTitles = content.printTitles?.find((titles) => titles.sheetId === sheetId);
  const savedPageBreaks = content.pageBreaks?.find((pageBreaks) => pageBreaks.sheetId === sheetId);
  const savedPageSetup = content.pageSetups?.find((pageSetup) => pageSetup.sheetId === sheetId);
  const [reference, setReference] = useState(savedArea?.reference ?? '');
  const [titleRows, setTitleRows] = useState(savedTitles?.rows ?? '');
  const [titleColumns, setTitleColumns] = useState(savedTitles?.columns ?? '');
  const [rowPageBreaks, setRowPageBreaks] = useState(formatSpreadsheetRowPageBreaks(savedPageBreaks?.rows));
  const [columnPageBreaks, setColumnPageBreaks] = useState(formatSpreadsheetColumnPageBreaks(savedPageBreaks?.columns));
  const [pageSetup, setPageSetup] = useState(() => effectiveSpreadsheetPageSetup(savedPageSetup));
  const [error, setError] = useState('');

  useEffect(() => {
    setReference(content.printAreas?.find((area) => area.sheetId === sheetId)?.reference ?? '');
    const titles = content.printTitles?.find((item) => item.sheetId === sheetId);
    setTitleRows(titles?.rows ?? '');
    setTitleColumns(titles?.columns ?? '');
    const pageBreaks = content.pageBreaks?.find((item) => item.sheetId === sheetId);
    setRowPageBreaks(formatSpreadsheetRowPageBreaks(pageBreaks?.rows));
    setColumnPageBreaks(formatSpreadsheetColumnPageBreaks(pageBreaks?.columns));
    setPageSetup(effectiveSpreadsheetPageSetup(content.pageSetups?.find((item) => item.sheetId === sheetId)));
    setError('');
  }, [content.pageBreaks, content.pageSetups, content.printAreas, content.printTitles, sheetId]);

  const saveSettings = () => {
    const sheet = availableSheets.find((item) => item.id === sheetId);
    const maximumRow = spreadsheetMaximumRow(sheet);
    const maximumColumn = spreadsheetMaximumColumn(sheet);
    const normalized = reference.trim() ? normalizeSpreadsheetPrintArea(reference) : null;
    const normalizedRows = titleRows.trim() ? normalizeSpreadsheetPrintTitleRows(titleRows) : null;
    const normalizedColumns = titleColumns.trim() ? normalizeSpreadsheetPrintTitleColumns(titleColumns) : null;
    const parsedRowPageBreaks = rowPageBreaks.trim() ? parseSpreadsheetRowPageBreaks(rowPageBreaks, maximumRow) : [];
    const parsedColumnPageBreaks = columnPageBreaks.trim()
      ? parseSpreadsheetColumnPageBreaks(columnPageBreaks, maximumColumn)
      : [];
    if (reference.trim() && !normalized) {
      setError('请输入有效的 A1 范围，例如 $A$1:$J$40。');
      return;
    }
    if (titleRows.trim() && !normalizedRows) {
      setError('重复标题行必须是整行范围，例如 $1:$2。');
      return;
    }
    if (titleColumns.trim() && !normalizedColumns) {
      setError('重复标题列必须是整列范围，例如 $A:$C。');
      return;
    }
    if (!parsedRowPageBreaks) {
      setError(`水平分页符必须是 2 到 ${maximumRow + 1} 之间的行号。`);
      return;
    }
    if (!parsedColumnPageBreaks) {
      setError('垂直分页符必须是工作表范围内、从 B 开始的列标。');
      return;
    }
    if (!validPageSetup(pageSetup)) {
      setError('缩放、适合页数、页码或页边距超出有效范围。');
      return;
    }

    const nextAreas = (content.printAreas ?? []).filter((area) => area.sheetId !== sheetId);
    if (normalized) nextAreas.push({ sheetId, reference: normalized });
    const nextTitles = (content.printTitles ?? []).filter((titles) => titles.sheetId !== sheetId);
    if (normalizedRows || normalizedColumns) {
      nextTitles.push({
        sheetId,
        rows: normalizedRows ?? undefined,
        columns: normalizedColumns ?? undefined,
      });
    }
    const nextPageBreaks = (content.pageBreaks ?? []).filter((pageBreaks) => pageBreaks.sheetId !== sheetId);
    if (parsedRowPageBreaks.length || parsedColumnPageBreaks.length) {
      nextPageBreaks.push({
        sheetId,
        rows: parsedRowPageBreaks.length ? parsedRowPageBreaks : undefined,
        columns: parsedColumnPageBreaks.length ? parsedColumnPageBreaks : undefined,
      });
    }
    const nextPageSetups = (content.pageSetups ?? []).filter((item) => item.sheetId !== sheetId);
    nextPageSetups.push({ sheetId, ...pageSetup });
    onChange({
      ...content,
      printAreas: nextAreas.length ? nextAreas : undefined,
      printTitles: nextTitles.length ? nextTitles : undefined,
      pageBreaks: nextPageBreaks.length ? nextPageBreaks : undefined,
      pageSetups: nextPageSetups,
    });
    setReference(normalized ?? '');
    setTitleRows(normalizedRows ?? '');
    setTitleColumns(normalizedColumns ?? '');
    setRowPageBreaks(formatSpreadsheetRowPageBreaks(parsedRowPageBreaks));
    setColumnPageBreaks(formatSpreadsheetColumnPageBreaks(parsedColumnPageBreaks));
    setError('');
  };

  const clearSettings = () => {
    const nextAreas = (content.printAreas ?? []).filter((area) => area.sheetId !== sheetId);
    const nextTitles = (content.printTitles ?? []).filter((titles) => titles.sheetId !== sheetId);
    const nextPageBreaks = (content.pageBreaks ?? []).filter((pageBreaks) => pageBreaks.sheetId !== sheetId);
    const nextPageSetups = (content.pageSetups ?? []).filter((pageSetup) => pageSetup.sheetId !== sheetId);
    onChange({
      ...content,
      printAreas: nextAreas.length ? nextAreas : undefined,
      printTitles: nextTitles.length ? nextTitles : undefined,
      pageBreaks: nextPageBreaks.length ? nextPageBreaks : undefined,
      pageSetups: nextPageSetups.length ? nextPageSetups : undefined,
    });
    setReference('');
    setTitleRows('');
    setTitleColumns('');
    setRowPageBreaks('');
    setColumnPageBreaks('');
    setPageSetup(effectiveSpreadsheetPageSetup(undefined));
    setError('');
  };

  return (
    <form
      className='work-spreadsheet-print-area-form'
      onSubmit={(event) => {
        event.preventDefault();
        saveSettings();
      }}
    >
      <label>
        <span>工作表</span>
        <select aria-label='打印设置工作表' value={sheetId} onChange={(event) => setSheetId(event.target.value)}>
          {availableSheets.map((sheet) => (
            <option value={sheet.id} key={sheet.id}>
              {sheet.name}
            </option>
          ))}
        </select>
      </label>
      <label className='reference'>
        <span>打印范围</span>
        <input
          aria-label='打印范围'
          value={reference}
          placeholder='$A$1:$J$40'
          onChange={(event) => setReference(event.target.value)}
        />
      </label>
      <label className='reference'>
        <span>重复标题行</span>
        <input
          aria-label='重复标题行'
          value={titleRows}
          placeholder='$1:$2'
          onChange={(event) => setTitleRows(event.target.value)}
        />
      </label>
      <label className='reference'>
        <span>重复标题列</span>
        <input
          aria-label='重复标题列'
          value={titleColumns}
          placeholder='$A:$C'
          onChange={(event) => setTitleColumns(event.target.value)}
        />
      </label>
      <label className='reference'>
        <span>手动水平分页符</span>
        <input
          aria-label='手动水平分页符'
          value={rowPageBreaks}
          placeholder='20, 35'
          onChange={(event) => setRowPageBreaks(event.target.value)}
        />
      </label>
      <label className='reference'>
        <span>手动垂直分页符</span>
        <input
          aria-label='手动垂直分页符'
          value={columnPageBreaks}
          placeholder='E, K'
          onChange={(event) => setColumnPageBreaks(event.target.value)}
        />
      </label>
      <fieldset className='work-spreadsheet-page-setup-fields'>
        <legend>页面设置与缩放</legend>
        <label>
          <span>纸张大小</span>
          <select
            aria-label='纸张大小'
            value={pageSetup.paperSize}
            onChange={(event) =>
              setPageSetup({
                ...pageSetup,
                paperSize: event.target.value as EffectiveSpreadsheetPageSetup['paperSize'],
              })
            }
          >
            <option value='a3'>A3</option>
            <option value='a4'>A4</option>
            <option value='a5'>A5</option>
            <option value='letter'>Letter</option>
            <option value='legal'>Legal</option>
            <option value='tabloid'>Tabloid</option>
          </select>
        </label>
        <label>
          <span>页面方向</span>
          <select
            aria-label='页面方向'
            value={pageSetup.orientation}
            onChange={(event) =>
              setPageSetup({
                ...pageSetup,
                orientation: event.target.value as EffectiveSpreadsheetPageSetup['orientation'],
              })
            }
          >
            <option value='landscape'>横向</option>
            <option value='portrait'>纵向</option>
          </select>
        </label>
        <label>
          <span>缩放方式</span>
          <select
            aria-label='缩放方式'
            value={pageSetup.fitToPage ? 'fit' : 'scale'}
            onChange={(event) => setPageSetup({ ...pageSetup, fitToPage: event.target.value === 'fit' })}
          >
            <option value='scale'>按比例缩放</option>
            <option value='fit'>适合指定页数</option>
          </select>
        </label>
        <label>
          <span>缩放比例（10–400%）</span>
          <input
            aria-label='缩放比例'
            type='number'
            min={10}
            max={400}
            disabled={pageSetup.fitToPage}
            value={pageSetup.scale}
            onChange={(event) => setPageSetup({ ...pageSetup, scale: Number(event.target.value) })}
          />
        </label>
        <label>
          <span>适合页宽（0 为自动）</span>
          <input
            aria-label='适合页宽'
            type='number'
            min={0}
            max={32767}
            disabled={!pageSetup.fitToPage}
            value={pageSetup.fitToWidth}
            onChange={(event) => setPageSetup({ ...pageSetup, fitToWidth: Number(event.target.value) })}
          />
        </label>
        <label>
          <span>适合页高（0 为自动）</span>
          <input
            aria-label='适合页高'
            type='number'
            min={0}
            max={32767}
            disabled={!pageSetup.fitToPage}
            value={pageSetup.fitToHeight}
            onChange={(event) => setPageSetup({ ...pageSetup, fitToHeight: Number(event.target.value) })}
          />
        </label>
        <PageMarginField
          label='上边距（毫米）'
          value={pageSetup.margins.top}
          onChange={(top) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, top } })}
        />
        <PageMarginField
          label='右边距（毫米）'
          value={pageSetup.margins.right}
          onChange={(right) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, right } })}
        />
        <PageMarginField
          label='下边距（毫米）'
          value={pageSetup.margins.bottom}
          onChange={(bottom) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, bottom } })}
        />
        <PageMarginField
          label='左边距（毫米）'
          value={pageSetup.margins.left}
          onChange={(left) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, left } })}
        />
        <PageMarginField
          label='页眉边距（毫米）'
          value={pageSetup.margins.header}
          onChange={(header) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, header } })}
        />
        <PageMarginField
          label='页脚边距（毫米）'
          value={pageSetup.margins.footer}
          onChange={(footer) => setPageSetup({ ...pageSetup, margins: { ...pageSetup.margins, footer } })}
        />
        <label className='toggle'>
          <input
            aria-label='水平居中'
            type='checkbox'
            checked={pageSetup.horizontalCentered}
            onChange={(event) => setPageSetup({ ...pageSetup, horizontalCentered: event.target.checked })}
          />
          <span>水平居中</span>
        </label>
        <label className='toggle'>
          <input
            aria-label='垂直居中'
            type='checkbox'
            checked={pageSetup.verticalCentered}
            onChange={(event) => setPageSetup({ ...pageSetup, verticalCentered: event.target.checked })}
          />
          <span>垂直居中</span>
        </label>
      </fieldset>
      <SpreadsheetHeaderFooterFields pageSetup={pageSetup} onChange={setPageSetup} />
      <p>范围、标题、分页符、页眉页脚与页面设置会保留到 XLSX，并共同控制分页 PDF。</p>
      <div className='actions'>
        {error && <output className='error'>{error}</output>}
        <button
          type='button'
          disabled={!savedArea && !savedTitles && !savedPageBreaks && !savedPageSetup}
          onClick={clearSettings}
        >
          清除
        </button>
        <button type='submit' className='primary' disabled={!sheetId}>
          保存打印设置
        </button>
      </div>
    </form>
  );
}

function spreadsheetMaximumRow(sheet: WorkSpreadsheetContent['sheets'][number] | undefined): number {
  return Math.max(0, (sheet?.row ?? 1) - 1, (sheet?.data?.length ?? 1) - 1);
}

function spreadsheetMaximumColumn(sheet: WorkSpreadsheetContent['sheets'][number] | undefined): number {
  let maximum = Math.max(0, (sheet?.column ?? 1) - 1);
  for (const row of sheet?.data ?? []) maximum = Math.max(maximum, row.length - 1);
  return maximum;
}

function PageMarginField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label>
      <span>{label}</span>
      <input
        aria-label={label}
        type='number'
        min={0}
        max={100}
        step='0.01'
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

function validPageSetup(pageSetup: EffectiveSpreadsheetPageSetup): boolean {
  const validInteger = (value: number, minimum: number, maximum: number) =>
    Number.isInteger(value) && value >= minimum && value <= maximum;
  return (
    validInteger(pageSetup.scale, 10, 400) &&
    validInteger(pageSetup.fitToWidth, 0, 32_767) &&
    validInteger(pageSetup.fitToHeight, 0, 32_767) &&
    validInteger(pageSetup.pageNumberStart, 1, 32_767) &&
    Object.values(pageSetup.margins).every((margin) => Number.isFinite(margin) && margin >= 0 && margin <= 100)
  );
}
