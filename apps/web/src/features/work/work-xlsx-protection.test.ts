import type { CellMatrix, Sheet } from '@fortune-sheet/core';
import { describe, expect, it } from 'vitest';
import { parseXml } from './work-ooxml-package';
import {
  applyPasswordlessEditableRanges,
  applySpreadsheetCellProtectionRanges,
  editableRangeRequiresCredentials,
} from './work-spreadsheet-protection';
import {
  diagnoseXlsxProtection,
  readXlsxProtection,
  writeXlsxProtection,
  XlsxCellProtectionWriter,
} from './work-xlsx-protection';

describe('Work XLSX sheet protection', () => {
  it('reads sheet permissions, protected ranges, and style-based cell protection', () => {
    const worksheet = parseXml(
      worksheetXml(`
        <dimension ref="A1:C3"/>
        <sheetViews><sheetView workbookViewId="0"/></sheetViews>
        <sheetFormatPr defaultRowHeight="15"/>
        <cols><col min="2" max="2" style="1"/></cols>
        <sheetData>
          <row r="1"><c r="A1" t="inlineStr"><is><t>Owner</t></is></c></row>
          <row r="2" s="2" customFormat="1">
            <c r="A2"><f>SUM(A3:A4)</f><v>1</v></c>
            <c r="C2" s="1"><v>2</v></c>
          </row>
        </sheetData>
        <sheetProtection sheet="1" objects="1" selectLockedCells="1" formatCells="0" autoFilter="0" password="ABCD"/>
        <protectedRanges>
          <protectedRange name="Public input" sqref="A3 B3"/>
          <protectedRange name="Managers" sqref="C3" password="1234"/>
        </protectedRanges>
      `),
      'worksheet fixture'
    );
    const styles = parseXml(stylesXml(), 'styles fixture');

    const result = readXlsxProtection(worksheet, styles);
    expect(result.authority).toMatchObject({
      sheet: 1,
      selectLockedCells: 0,
      selectunLockedCells: 1,
      formatCells: 1,
      filter: 1,
      editObjects: 0,
      allowRangeList: [
        { name: 'Public input', sqref: 'A3,B3' },
        { name: 'Managers', sqref: 'C3' },
      ],
      xlsxAttributes: { password: 'ABCD' },
    });
    expect(editableRangeRequiresCredentials(result.authority!.allowRangeList[0])).toBe(false);
    expect(editableRangeRequiresCredentials(result.authority!.allowRangeList[1])).toBe(true);
    expect(result.cellProtectionRanges).toEqual([
      { range: { row: [0, 2], column: [1, 1] }, locked: false, hidden: false },
      { range: { row: [1, 1], column: [0, 2] }, locked: true, hidden: true },
      { range: { row: [1, 1], column: [2, 2] }, locked: false, hidden: false },
    ]);

    const data: CellMatrix = Array.from({ length: 3 }, () => Array(3).fill(null));
    applySpreadsheetCellProtectionRanges(data, result.cellProtectionRanges, 3, 3);
    applyPasswordlessEditableRanges(data, result.authority!.allowRangeList, 3, 3);
    expect(data[0][1]).toMatchObject({ lo: 0 });
    expect(data[1][0]).toMatchObject({ lo: 1, hi: 1 });
    expect(data[1][2]).toMatchObject({ lo: 0 });
    expect(data[2][0]).toMatchObject({ lo: 0 });
    expect(data[2][1]).toMatchObject({ lo: 0 });
    expect(data[2][2]).toBeNull();

    expect(diagnoseXlsxProtection(worksheet, styles).map((item) => item.code)).toEqual([
      'xlsx.protection',
      'xlsx.protection.password',
      'xlsx.protection.range-credentials',
      'xlsx.protection.permissions',
      'xlsx.protection.hidden-formulas',
    ]);
  });

  it('writes enforceable locks, unlocked cells, source verifiers, and editable ranges', () => {
    const worksheet = parseXml(
      worksheetXml(`
        <dimension ref="A1"/>
        <sheetViews><sheetView workbookViewId="0"/></sheetViews>
        <sheetFormatPr defaultRowHeight="15"/>
        <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
        <pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/>
      `),
      'worksheet fixture'
    );
    const styles = parseXml(
      `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
        <cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>
      </styleSheet>`,
      'styles fixture'
    );
    const sheet: Sheet = {
      id: 'sheet-1',
      name: 'Inputs',
      row: 2,
      column: 2,
      data: [
        [{ v: 1, lo: 0 }, null],
        [null, { f: '=A1', v: 1, lo: 1, hi: 1 } as never],
      ],
      config: {
        authority: {
          sheet: 1,
          selectLockedCells: 0,
          selectunLockedCells: 1,
          formatCells: 1,
          formatColumns: 0,
          formatRows: 0,
          insertColumns: 0,
          insertRows: 0,
          insertHyperlinks: 0,
          deleteColumns: 0,
          deleteRows: 0,
          sort: 0,
          filter: 1,
          usePivotTablereports: 0,
          editObjects: 0,
          editScenarios: 0,
          hintText: '',
          defaultSheetHintText: 'Protected',
          allowRangeList: [
            { name: 'Public input', sqref: 'A1:A2' },
            {
              name: 'Managers',
              sqref: 'B2',
              xlsxAttributes: { password: '1234' },
            },
          ],
          xlsxAttributes: { password: 'ABCD' },
        },
      },
    };

    const writer = new XlsxCellProtectionWriter(styles);
    writeXlsxProtection(worksheet, sheet, writer);
    const sheetXml = new XMLSerializer().serializeToString(worksheet);
    const outputStyles = writer.serialize();

    expect(sheetXml).toContain(
      '<sheetProtection password="ABCD" sheet="1" selectLockedCells="1" selectUnlockedCells="0"'
    );
    expect(sheetXml).toContain('formatCells="0"');
    expect(sheetXml).toContain('autoFilter="0"');
    expect(sheetXml).toContain('<protectedRange name="Public input" sqref="A1:A2"/>');
    expect(sheetXml).toContain('<protectedRange password="1234" name="Managers" sqref="B2"/>');
    expect(sheetXml).toContain('<c r="B2" s="2"/>');
    expect(outputStyles).toContain('<cellXfs count="3">');
    expect(outputStyles).toContain('<protection locked="0" hidden="0"/>');
    expect(outputStyles).toContain('<protection locked="1" hidden="1"/>');
    expect(Array.from(worksheet.documentElement.children).map((element) => element.localName)).toEqual([
      'dimension',
      'sheetViews',
      'sheetFormatPr',
      'sheetData',
      'sheetProtection',
      'protectedRanges',
      'pageMargins',
    ]);

    const reopened = readXlsxProtection(worksheet, styles);
    expect(reopened.authority).toMatchObject({
      sheet: 1,
      selectLockedCells: 0,
      selectunLockedCells: 1,
      formatCells: 1,
      filter: 1,
    });
    expect(reopened.cellProtectionRanges.at(-2)).toMatchObject({
      range: { row: [0, 0], column: [0, 0] },
      locked: false,
      hidden: false,
    });
    expect(reopened.cellProtectionRanges.at(-1)).toMatchObject({
      range: { row: [1, 1], column: [1, 1] },
      locked: true,
      hidden: true,
    });
  });
});

function worksheetXml(children: string): string {
  return `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">${children}</worksheet>`;
}

function stylesXml(): string {
  return `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <cellXfs count="3">
      <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
      <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyProtection="1">
        <protection locked="0"/>
      </xf>
      <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyProtection="1">
        <protection locked="1" hidden="1"/>
      </xf>
    </cellXfs>
  </styleSheet>`;
}
