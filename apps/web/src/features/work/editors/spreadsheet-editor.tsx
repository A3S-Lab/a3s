import type { Hooks, Selection } from '@fortune-sheet/core';
import { Workbook, type WorkbookInstance } from '@fortune-sheet/react';
import '@fortune-sheet/react/dist/index.css';
import { BarChart3, Bookmark, Calculator, Palette, Printer, ShieldCheck, TableProperties } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { WorkspaceContextMenu } from '../../workspace/components/workspace-context-menu';
import { spreadsheetAgentMenuItems } from '../components/work-editor-agent-menus';
import { applySpreadsheetAgentProposalChanges } from '../work-agent-proposal-apply';
import type { WorkEditorAgentRequest } from '../work-agent-request';
import { spreadsheetAgentSelection, type WorkSpreadsheetAgentSelection } from '../work-spreadsheet-agent-context';
import {
  drawSpreadsheetCommentMarker,
  drawSpreadsheetConditionalDataBar,
} from '../work-spreadsheet-conditional-canvas';
import { drawSpreadsheetConditionalIcon } from '../work-spreadsheet-conditional-icons';
import { spreadsheetConditionalFormatStyles } from '../work-spreadsheet-conditional-format';
import {
  reconcileSpreadsheetChartPreviews,
  spreadsheetChartCount,
  spreadsheetSheetsWithChartPreviews,
} from '../work-spreadsheet-charts';
import { spreadsheetFormulaCount } from '../work-spreadsheet-formula-analysis';
import {
  reconcileSpreadsheetPivots,
  refreshSpreadsheetPivotTables,
  spreadsheetPivotCount,
  spreadsheetPivotIntersects,
  spreadsheetPivotOutputContains,
} from '../work-spreadsheet-pivots';
import { protectedSheetCount, spreadsheetProtectionKey } from '../work-spreadsheet-protection';
import type { WorkSpreadsheetContent } from '../work-types';
import { managedConditionalFormatCount } from './spreadsheet-conditional-format-panel';
import { spreadsheetPrintSettingCount } from './spreadsheet-print-settings-panel';
import { SpreadsheetWorkbookPanel, type SpreadsheetWorkbookPanelView } from './spreadsheet-workbook-panel';

interface SpreadsheetEditorProps {
  content: WorkSpreadsheetContent;
  preview: boolean;
  onChange: (content: WorkSpreadsheetContent) => void;
  onAgentRequest?: (request: WorkEditorAgentRequest) => void | Promise<void>;
}

interface SpreadsheetSelectionState {
  sheetId: string;
  selection: Selection;
}

interface SpreadsheetAgentMenuState {
  x: number;
  y: number;
  selection: WorkSpreadsheetAgentSelection;
}

export function SpreadsheetEditor({ content, preview, onChange, onAgentRequest }: SpreadsheetEditorProps) {
  const materializedContent = useMemo(() => refreshSpreadsheetPivotTables(content), [content]);
  const contentRef = useRef(materializedContent);
  const workbookRef = useRef<WorkbookInstance>(null);
  const [panel, setPanel] = useState<SpreadsheetWorkbookPanelView | null>(null);
  const [selectionState, setSelectionState] = useState<SpreadsheetSelectionState | null>(null);
  const [agentMenu, setAgentMenu] = useState<SpreadsheetAgentMenuState | null>(null);
  const activeSheetId =
    content.sheets.find((sheet) => sheet.status === 1)?.id ?? content.sheets.find((sheet) => !sheet.hide)?.id ?? '';
  const activeSheetIdRef = useRef(activeSheetId);
  contentRef.current = materializedContent;
  const conditionalStylesBySheet = useMemo(
    () =>
      new Map(
        materializedContent.sheets.flatMap((sheet) =>
          sheet.id ? [[sheet.id, spreadsheetConditionalFormatStyles(sheet)] as const] : []
        )
      ),
    [materializedContent.sheets]
  );
  useEffect(() => {
    activeSheetIdRef.current = activeSheetId;
  }, [activeSheetId]);
  const workbookHooks = useMemo<Hooks>(
    () => ({
      afterActivateSheet: (id) => {
        activeSheetIdRef.current = id;
        setSelectionState(null);
      },
      afterSelectionChange: (sheetId, selection) => {
        setSelectionState({ sheetId, selection });
      },
      beforeUpdateCell: (row, column) => {
        const sheet = contentRef.current.sheets.find((candidate) => candidate.id === activeSheetIdRef.current);
        return !sheet || !spreadsheetPivotOutputContains(sheet, row, column);
      },
      beforePaste: (selections) => {
        const sheet = contentRef.current.sheets.find((candidate) => candidate.id === activeSheetIdRef.current);
        if (!sheet) return true;
        return !(selections ?? []).some(
          (selection) =>
            spreadsheetPivotIntersects(sheet, {
              startRow: Math.min(selection.row[0], selection.row[1]),
              endRow: Math.max(selection.row[0], selection.row[1]),
              startColumn: Math.min(selection.column[0], selection.column[1]),
              endColumn: Math.max(selection.column[0], selection.column[1]),
            }).length
        );
      },
      beforeRenderCell: (_cell, cellInfo, context) => {
        const style = conditionalStylesBySheet.get(activeSheetIdRef.current)?.get(`${cellInfo.row}_${cellInfo.column}`);
        if (style?.cellColor) context.fillStyle = style.cellColor;
        return true;
      },
      afterRenderCell: (cell, cellInfo, context) => {
        const style = conditionalStylesBySheet.get(activeSheetIdRef.current)?.get(`${cellInfo.row}_${cellInfo.column}`);
        if (!style?.icon && !style?.dataBar) return;
        const background = style.cellColor ?? (typeof cell?.bg === 'string' ? cell.bg : '#ffffff');
        if (style.dataBar) {
          drawSpreadsheetConditionalDataBar(
            context,
            cellInfo,
            {
              ...style.dataBar,
              showValue: style.dataBar.showValue && (style.icon?.showValue ?? true),
            },
            cell,
            background,
            style.textColor
          );
        }
        if (style.icon) {
          drawSpreadsheetConditionalIcon(
            context,
            cellInfo,
            style.icon,
            background,
            style.dataBar ? false : !style.icon.showValue
          );
        }
        if (cell?.ps) drawSpreadsheetCommentMarker(context, cellInfo);
      },
    }),
    [conditionalStylesBySheet]
  );
  const conditionalFormatKey = content.sheets
    .map((sheet) => `${sheet.id}:${JSON.stringify(sheet.luckysheet_conditionformat_save ?? [])}`)
    .join('|');
  const protectionKey = spreadsheetProtectionKey(content.sheets);
  const printSettingCount = spreadsheetPrintSettingCount(content);
  const formulaCount = useMemo(() => spreadsheetFormulaCount(materializedContent), [materializedContent]);
  const pivotCount = useMemo(() => spreadsheetPivotCount(materializedContent), [materializedContent]);
  const workbookSheets = useMemo(() => spreadsheetSheetsWithChartPreviews(materializedContent), [materializedContent]);
  const chartPreviewKey = workbookSheets
    .flatMap((sheet) =>
      (sheet.images ?? [])
        .filter((image) => image.id.startsWith('work-chart-preview-'))
        .map((image) => `${image.id}:${image.src}:${image.left}:${image.top}:${image.width}:${image.height}`)
    )
    .join('|');
  const panelSheetId = selectionState?.sheetId ?? activeSheetIdRef.current ?? activeSheetId;
  const recalculate = (scope: 'workbook' | 'selection'): boolean => {
    const workbook = workbookRef.current;
    if (!workbook) return false;
    if (scope === 'workbook') {
      workbook.calculateFormula();
      return true;
    }
    if (!selectionState) return false;
    if (selectionState.selection.row.length < 2 || selectionState.selection.column.length < 2) return false;
    const rowStart = Math.min(selectionState.selection.row[0], selectionState.selection.row[1]);
    const rowEnd = Math.max(selectionState.selection.row[0], selectionState.selection.row[1]);
    const columnStart = Math.min(selectionState.selection.column[0], selectionState.selection.column[1]);
    const columnEnd = Math.max(selectionState.selection.column[0], selectionState.selection.column[1]);
    workbook.calculateFormula(selectionState.sheetId, {
      row: [rowStart, rowEnd],
      column: [columnStart, columnEnd],
    });
    return true;
  };
  return (
    <section className={`work-spreadsheet-editor ${preview ? 'preview' : ''}`} aria-label='表格工作区'>
      {preview && <div className='work-preview-notice'>只读预览 · {content.sheets.length} 个工作表</div>}
      {!preview && (
        <div className='work-spreadsheet-metadata-toolbar' role='toolbar' aria-label='工作簿工具'>
          <button
            type='button'
            className={panel === 'names' ? 'active' : ''}
            aria-pressed={panel === 'names'}
            onClick={() => setPanel((value) => (value === 'names' ? null : 'names'))}
          >
            <Bookmark size={14} />
            名称管理器
            <span>{content.namedRanges?.length ?? 0}</span>
          </button>
          <button
            type='button'
            className={panel === 'print-area' ? 'active' : ''}
            aria-pressed={panel === 'print-area'}
            onClick={() => setPanel((value) => (value === 'print-area' ? null : 'print-area'))}
          >
            <Printer size={14} />
            打印设置
            <span>{printSettingCount}</span>
          </button>
          <button
            type='button'
            className={panel === 'conditional-formatting' ? 'active' : ''}
            aria-pressed={panel === 'conditional-formatting'}
            onClick={() => setPanel((value) => (value === 'conditional-formatting' ? null : 'conditional-formatting'))}
          >
            <Palette size={14} />
            条件格式
            <span>{managedConditionalFormatCount(content)}</span>
          </button>
          <button
            type='button'
            className={panel === 'formulas' ? 'active' : ''}
            aria-pressed={panel === 'formulas'}
            onClick={() => setPanel((value) => (value === 'formulas' ? null : 'formulas'))}
          >
            <Calculator size={14} />
            公式与计算
            <span>{formulaCount}</span>
          </button>
          <button
            type='button'
            className={panel === 'charts' ? 'active' : ''}
            aria-pressed={panel === 'charts'}
            onClick={() => setPanel((value) => (value === 'charts' ? null : 'charts'))}
          >
            <BarChart3 size={14} />
            图表
            <span>{spreadsheetChartCount(content)}</span>
          </button>
          <button
            type='button'
            className={panel === 'pivots' ? 'active' : ''}
            aria-pressed={panel === 'pivots'}
            onClick={() => setPanel((value) => (value === 'pivots' ? null : 'pivots'))}
          >
            <TableProperties size={14} />
            数据透视表
            <span>{pivotCount}</span>
          </button>
          <button
            type='button'
            className={panel === 'protection' ? 'active' : ''}
            aria-pressed={panel === 'protection'}
            onClick={() => setPanel((value) => (value === 'protection' ? null : 'protection'))}
          >
            <ShieldCheck size={14} />
            工作表保护
            <span>{protectedSheetCount(content.sheets)}</span>
          </button>
        </div>
      )}
      {!preview && panel && (
        <SpreadsheetWorkbookPanel
          content={materializedContent}
          view={panel}
          activeSheetId={panelSheetId}
          selection={selectionState?.sheetId === panelSheetId ? selectionState.selection : undefined}
          onChange={onChange}
          onRecalculate={recalculate}
          onClose={() => setPanel(null)}
        />
      )}
      <div
        className='work-spreadsheet-canvas'
        onContextMenuCapture={(event) => {
          if (preview || !onAgentRequest) return;
          const sheetId = selectionState?.sheetId ?? activeSheetIdRef.current;
          const sheet = content.sheets.find((candidate) => candidate.id === sheetId);
          const selection = selectionState?.selection ?? sheet?.luckysheet_select_save?.at(-1);
          if (!selection) return;
          const agentSelection = spreadsheetAgentSelection(content, sheetId, selection);
          if (!agentSelection) return;
          event.preventDefault();
          event.stopPropagation();
          setAgentMenu({
            x: event.clientX,
            y: event.clientY,
            selection: agentSelection,
          });
        }}
      >
        <Workbook
          ref={workbookRef}
          key={`${preview ? 'spreadsheet-preview' : 'spreadsheet-edit'}:${conditionalFormatKey}:${protectionKey}:${chartPreviewKey}`}
          data={workbookSheets}
          lang='zh'
          allowEdit={!preview}
          showToolbar={!preview}
          showFormulaBar
          showSheetTabs
          row={60}
          column={26}
          defaultRowHeight={24}
          defaultColWidth={96}
          hooks={workbookHooks}
          onChange={(sheets) => {
            if (!preview) {
              const withCharts = reconcileSpreadsheetChartPreviews(contentRef.current, sheets);
              onChange(reconcileSpreadsheetPivots(contentRef.current, withCharts.sheets));
            }
          }}
        />
      </div>
      {agentMenu && onAgentRequest && (
        <WorkspaceContextMenu
          label={`表格选区 ${agentMenu.selection.reference} AI 操作`}
          x={agentMenu.x}
          y={agentMenu.y}
          items={spreadsheetAgentMenuItems(agentMenu.selection, onAgentRequest, (changes) => {
            const outcome = applySpreadsheetAgentProposalChanges(
              contentRef.current,
              agentMenu.selection.sheetId,
              changes
            );
            if (outcome.result.appliedTargetIds.length) onChange(outcome.content);
            return outcome.result;
          })}
          onClose={() => setAgentMenu(null)}
        />
      )}
    </section>
  );
}
