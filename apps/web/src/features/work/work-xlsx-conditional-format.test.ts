import { describe, expect, it } from 'vitest';
import { directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import {
  readXlsxConditionalFormats,
  readXlsxDifferentialFormats,
  writeXlsxConditionalFormats,
  XlsxDifferentialFormatWriter,
} from './work-xlsx-conditional-format';
import { diagnoseXlsxConditionalFormatting } from './work-xlsx-conditional-format-diagnostics';

const SPREADSHEET_NAMESPACE = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main';

describe('XLSX conditional formatting interoperability', () => {
  it('imports common cell, scale, data-bar, and icon-set rules without borrowing a missing differential style', () => {
    const styles = parseXml(`
      <styleSheet xmlns="${SPREADSHEET_NAMESPACE}">
        <dxfs count="2">
          <dxf>
            <font><color rgb="FFFF0000"/></font>
            <fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill>
          </dxf>
          <dxf><font><color rgb="FF0000FF"/></font></dxf>
        </dxfs>
      </styleSheet>
    `);
    const worksheet = parseXml(`
      <worksheet xmlns="${SPREADSHEET_NAMESPACE}">
        <sheetData/>
        <conditionalFormatting sqref="A1:A4 C2">
          <cfRule type="containsText" priority="1" text="Ready">
            <formula>NOT(ISERROR(SEARCH("Ready",A1)))</formula>
          </cfRule>
          <cfRule type="cellIs" operator="greaterThan" dxfId="1" priority="2">
            <formula>10</formula>
          </cfRule>
          <cfRule type="duplicateValues" dxfId="0" priority="3"/>
        </conditionalFormatting>
        <conditionalFormatting sqref="B1:B4">
          <cfRule type="colorScale" priority="4">
            <colorScale>
              <cfvo type="num" val="0"/><cfvo type="percentile" val="75"/><cfvo type="num" val="120"/>
              <color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/>
            </colorScale>
          </cfRule>
          <cfRule type="dataBar" priority="5">
            <dataBar showValue="0" minLength="15" maxLength="70">
              <cfvo type="num" val="-20"/><cfvo type="percent" val="80"/><color rgb="FF5B9BD5"/>
            </dataBar>
          </cfRule>
          <cfRule type="iconSet" priority="6">
            <iconSet iconSet="3TrafficLights1" showValue="0" reverse="1">
              <cfvo type="min"/>
              <cfvo type="percent" val="33" gte="0"/>
              <cfvo type="percent" val="67"/>
            </iconSet>
          </cfRule>
        </conditionalFormatting>
      </worksheet>
    `);

    const rules = readXlsxConditionalFormats(worksheet, readXlsxDifferentialFormats(styles));

    expect(rules).toEqual([
      {
        type: 'default',
        cellrange: [
          { row: [0, 3], column: [0, 0] },
          { row: [1, 1], column: [2, 2] },
        ],
        format: { textColor: null, cellColor: null },
        conditionName: 'textContains',
        conditionRange: [],
        conditionValue: ['Ready'],
      },
      expect.objectContaining({
        type: 'default',
        format: { textColor: '#0000ff', cellColor: null },
        conditionName: 'greaterThan',
        conditionValue: ['10'],
      }),
      expect.objectContaining({
        type: 'default',
        format: { textColor: '#ff0000', cellColor: '#ffff00' },
        conditionName: 'duplicateValue',
        conditionValue: ['0'],
      }),
      {
        type: 'colorGradation',
        cellrange: [{ row: [0, 3], column: [1, 1] }],
        format: ['rgb(99, 190, 123)', 'rgb(255, 235, 132)', 'rgb(248, 105, 107)'],
        visualOptions: {
          thresholds: [
            { type: 'num', value: 0 },
            { type: 'percentile', value: 75 },
            { type: 'num', value: 120 },
          ],
        },
      },
      {
        type: 'dataBar',
        cellrange: [{ row: [0, 3], column: [1, 1] }],
        format: { textColor: null, cellColor: '#5b9bd5' },
        visualOptions: {
          thresholds: [
            { type: 'num', value: -20 },
            { type: 'percent', value: 80 },
          ],
          showValue: false,
          minLength: 15,
          maxLength: 70,
        },
      },
      {
        type: 'icons',
        cellrange: [{ row: [0, 3], column: [1, 1] }],
        format: {
          iconSet: '3TrafficLights1',
          showValue: false,
          reverse: true,
          percent: true,
          thresholds: [
            { type: 'min', gte: true },
            { type: 'percent', value: 33, gte: false },
            { type: 'percent', value: 67, gte: true },
          ],
        },
      },
    ]);
  });

  it('round-trips all core cell comparison operators', () => {
    const comparisons = [
      ['greaterThan', 'greaterThan', ['10']],
      ['lessThan', 'lessThan', ['10']],
      ['equal', 'equal', ['10']],
      ['between', 'between', ['5', '15']],
      ['notEqual', 'notEqual', ['10']],
      ['greaterThanOrEqual', 'greaterThanOrEqual', ['10']],
      ['lessThanOrEqual', 'lessThanOrEqual', ['10']],
      ['notBetween', 'notBetween', ['5', '15']],
    ] as const;
    const worksheet = parseXml(`
      <worksheet xmlns="${SPREADSHEET_NAMESPACE}">
        <sheetData/>
        <conditionalFormatting sqref="A1:A4">
          ${comparisons
            .map(
              ([operator, , formulas], index) =>
                `<cfRule type="cellIs" operator="${operator}" priority="${index + 1}">${formulas
                  .map((formula) => `<formula>${formula}</formula>`)
                  .join('')}</cfRule>`
            )
            .join('')}
        </conditionalFormatting>
      </worksheet>
    `);

    const rules = readXlsxConditionalFormats(worksheet, []);

    expect(
      rules.map((rule) => ({
        conditionName: rule.conditionName,
        conditionValue: rule.conditionValue,
      }))
    ).toEqual(
      comparisons.map(([, conditionName, conditionValue]) => ({
        conditionName,
        conditionValue: [...conditionValue],
      }))
    );
    expect(diagnoseXlsxConditionalFormatting(worksheet, null)).toEqual([
      expect.objectContaining({
        code: 'xlsx.conditional-formatting',
        severity: 'info',
        message: expect.stringContaining('8 supported'),
      }),
    ]);

    const output = parseXml(`<worksheet xmlns="${SPREADSHEET_NAMESPACE}"><sheetData/></worksheet>`);
    writeXlsxConditionalFormats(output, rules);
    const outputRules = directChildren(output.documentElement, 'conditionalFormatting').map((container) =>
      directChild(container, 'cfRule')
    );
    expect(outputRules.map((rule) => rule?.getAttribute('operator'))).toEqual(
      comparisons.map(([operator]) => operator)
    );
    expect(readXlsxConditionalFormats(output, [])).toEqual(rules);
  });

  it('orders imported rules by priority and round-trips stop-if-true semantics', () => {
    const worksheet = parseXml(`
      <worksheet xmlns="${SPREADSHEET_NAMESPACE}">
        <sheetData/>
        <conditionalFormatting sqref="A1:A4">
          <cfRule type="cellIs" operator="equal" priority="3"><formula>30</formula></cfRule>
          <cfRule type="cellIs" operator="greaterThan" priority="1" stopIfTrue="1">
            <formula>10</formula>
          </cfRule>
          <cfRule type="cellIs" operator="lessThan" priority="2"><formula>20</formula></cfRule>
        </conditionalFormatting>
      </worksheet>
    `);

    const rules = readXlsxConditionalFormats(worksheet, []);

    expect(rules.map((rule) => rule.conditionName)).toEqual(['greaterThan', 'lessThan', 'equal']);
    expect(rules[0]).toMatchObject({ stopIfTrue: true, conditionValue: ['10'] });
    expect(diagnoseXlsxConditionalFormatting(worksheet, null)).toEqual([
      expect.objectContaining({
        code: 'xlsx.conditional-formatting',
        severity: 'info',
        message: expect.stringContaining('3 supported'),
      }),
    ]);

    const output = parseXml(`<worksheet xmlns="${SPREADSHEET_NAMESPACE}"><sheetData/></worksheet>`);
    writeXlsxConditionalFormats(output, rules);
    const outputRules = directChildren(output.documentElement, 'conditionalFormatting').map((container) =>
      directChild(container, 'cfRule')
    );
    expect(outputRules.map((rule) => rule?.getAttribute('priority'))).toEqual(['1', '2', '3']);
    expect(outputRules.map((rule) => rule?.getAttribute('stopIfTrue'))).toEqual(['1', null, null]);
    expect(readXlsxConditionalFormats(output, [])).toEqual(rules);
  });

  it('writes valid OOXML ordering and round-trips supported FortuneSheet rules', () => {
    const styles = parseXml(
      `<styleSheet xmlns="${SPREADSHEET_NAMESPACE}"><dxfs count="0"/><tableStyles count="0"/></styleSheet>`
    );
    const worksheet = parseXml(
      `<worksheet xmlns="${SPREADSHEET_NAMESPACE}"><sheetData/><dataValidations count="0"/></worksheet>`
    );
    const differentialFormats = new XlsxDifferentialFormatWriter(styles);

    writeXlsxConditionalFormats(
      worksheet,
      [
        {
          type: 'default',
          cellrange: [{ row: [0, 3], column: [0, 0] }],
          format: { textColor: '#ffffff', cellColor: '#c00000' },
          conditionName: 'greaterThan',
          conditionRange: [],
          conditionValue: ['10'],
        },
        {
          type: 'default',
          cellrange: [{ row: [0, 3], column: [1, 1] }],
          format: { textColor: null, cellColor: '#fff2cc' },
          conditionName: 'duplicateValue',
          conditionRange: [],
          conditionValue: ['1'],
        },
        {
          type: 'colorGradation',
          cellrange: [{ row: [0, 3], column: [2, 2] }],
          format: ['rgb(99, 190, 123)', 'rgb(255, 235, 132)', 'rgb(248, 105, 107)'],
          visualOptions: {
            thresholds: [
              { type: 'num', value: 0 },
              { type: 'percentile', value: 60 },
              { type: 'num', value: 90 },
            ],
          },
        },
        {
          type: 'dataBar',
          cellrange: [{ row: [0, 3], column: [3, 3] }],
          format: { textColor: null, cellColor: '#5b9bd5' },
          visualOptions: {
            thresholds: [
              { type: 'num', value: -10 },
              { type: 'percent', value: 90 },
            ],
            showValue: false,
            minLength: 20,
            maxLength: 80,
          },
        },
        {
          type: 'icons',
          cellrange: [{ row: [0, 3], column: [4, 4] }],
          format: {
            iconSet: '4Arrows',
            showValue: false,
            reverse: true,
            percent: false,
            thresholds: [
              { type: 'min', gte: true },
              { type: 'num', value: 10, gte: true },
              { type: 'num', value: 20, gte: false },
              { type: 'max', gte: true },
            ],
          },
        },
      ],
      differentialFormats
    );

    const containers = directChildren(worksheet.documentElement, 'conditionalFormatting');
    expect(containers).toHaveLength(5);
    expect(directChildren(worksheet.documentElement).map((element) => element.localName)).toEqual([
      'sheetData',
      'conditionalFormatting',
      'conditionalFormatting',
      'conditionalFormatting',
      'conditionalFormatting',
      'conditionalFormatting',
      'dataValidations',
    ]);
    const colorScale = firstDescendant(containers[2], 'colorScale');
    expect(directChildren(colorScale!).map((element) => element.localName)).toEqual([
      'cfvo',
      'cfvo',
      'cfvo',
      'color',
      'color',
      'color',
    ]);
    expect(directChildren(colorScale!, 'cfvo').map((element) => element.getAttribute('type'))).toEqual([
      'num',
      'percentile',
      'num',
    ]);
    expect(firstDescendant(containers[3], 'dataBar')?.getAttribute('showValue')).toBe('0');
    expect(firstDescendant(containers[3], 'dataBar')?.getAttribute('minLength')).toBe('20');
    expect(firstDescendant(containers[3], 'dataBar')?.getAttribute('maxLength')).toBe('80');
    const dxfs = directChild(styles.documentElement, 'dxfs');
    expect(dxfs?.getAttribute('count')).toBe('2');
    expect(directChildren(dxfs!, 'dxf')).toHaveLength(2);

    expect(readXlsxConditionalFormats(worksheet, readXlsxDifferentialFormats(styles))).toEqual([
      expect.objectContaining({
        type: 'default',
        conditionName: 'greaterThan',
        conditionValue: ['10'],
        format: { textColor: '#ffffff', cellColor: '#c00000' },
      }),
      expect.objectContaining({
        type: 'default',
        conditionName: 'duplicateValue',
        conditionValue: ['1'],
        format: { textColor: null, cellColor: '#fff2cc' },
      }),
      expect.objectContaining({
        type: 'colorGradation',
        format: ['rgb(99, 190, 123)', 'rgb(255, 235, 132)', 'rgb(248, 105, 107)'],
        visualOptions: {
          thresholds: [
            { type: 'num', value: 0 },
            { type: 'percentile', value: 60 },
            { type: 'num', value: 90 },
          ],
        },
      }),
      expect.objectContaining({
        type: 'dataBar',
        format: { textColor: null, cellColor: '#5b9bd5' },
        visualOptions: {
          thresholds: [
            { type: 'num', value: -10 },
            { type: 'percent', value: 90 },
          ],
          showValue: false,
          minLength: 20,
          maxLength: 80,
        },
      }),
      {
        type: 'icons',
        cellrange: [{ row: [0, 3], column: [4, 4] }],
        format: {
          iconSet: '4Arrows',
          showValue: false,
          reverse: true,
          percent: false,
          thresholds: [
            { type: 'min', gte: true },
            { type: 'num', value: 10, gte: true },
            { type: 'num', value: 20, gte: false },
            { type: 'max', gte: true },
          ],
        },
      },
    ]);
  });

  it('distinguishes editable rules from unsupported and normalized XLSX features', () => {
    const styles = parseXml(`
      <styleSheet xmlns="${SPREADSHEET_NAMESPACE}">
        <dxfs count="1">
          <dxf><font><b/><color theme="1" tint="0.25"/></font><border/></dxf>
        </dxfs>
      </styleSheet>
    `);
    const worksheet = parseXml(`
      <worksheet xmlns="${SPREADSHEET_NAMESPACE}">
        <sheetData/>
        <conditionalFormatting sqref="A1:A4">
          <cfRule type="expression" dxfId="0" priority="1" stopIfTrue="1">
            <formula>INDIRECT("A1")&gt;0</formula>
          </cfRule>
          <cfRule type="cellIs" operator="notEqual" priority="2"><formula>0</formula></cfRule>
          <cfRule type="iconSet" priority="3">
            <iconSet iconSet="3TrafficLights1" showValue="0" reverse="1"/>
          </cfRule>
          <cfRule type="iconSet" priority="4">
            <iconSet iconSet="3Arrows" custom="1">
              <cfvo type="min"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/>
              <cfIcon iconSet="3Flags" iconId="0"/>
            </iconSet>
          </cfRule>
          <cfRule type="colorScale" priority="5">
            <colorScale>
              <cfvo type="num" val="0"/><cfvo type="num" val="100"/>
              <color theme="4"/><color indexed="10"/>
            </colorScale>
          </cfRule>
        </conditionalFormatting>
        <extLst>
          <ext>
            <conditionalFormatting>
              <cfRule type="dataBar">
                <dataBar direction="rightToLeft" axisPosition="middle" gradient="0">
                  <negativeFillColor rgb="FFFF0000"/><axisColor rgb="FF000000"/>
                </dataBar>
              </cfRule>
            </conditionalFormatting>
          </ext>
        </extLst>
      </worksheet>
    `);

    const diagnostics = diagnoseXlsxConditionalFormatting(worksheet, styles);
    const codes = diagnostics.map((diagnostic) => diagnostic.code);

    expect(codes).toEqual(
      expect.arrayContaining([
        'xlsx.conditional-formatting',
        'xlsx.conditional-formatting.unsupported',
        'xlsx.conditional-formatting.colors',
        'xlsx.conditional-formatting.formulas',
        'xlsx.conditional-formatting.options',
      ])
    );
    expect(diagnostics.find((diagnostic) => diagnostic.code === 'xlsx.conditional-formatting')).toMatchObject({
      severity: 'info',
    });
    expect(
      diagnostics.find((diagnostic) => diagnostic.code === 'xlsx.conditional-formatting.unsupported')?.message
    ).toContain('custom icon sets');
    expect(
      diagnostics.find((diagnostic) => diagnostic.code === 'xlsx.conditional-formatting.unsupported')?.message
    ).not.toContain('3TrafficLights1');
    expect(
      diagnostics.find((diagnostic) => diagnostic.code === 'xlsx.conditional-formatting.unsupported')?.message
    ).toContain('x14 data-bar direction');
  });

  it('treats explicit core scale and data-bar thresholds and display lengths as editable', () => {
    const worksheet = parseXml(`
      <worksheet xmlns="${SPREADSHEET_NAMESPACE}">
        <sheetData/>
        <conditionalFormatting sqref="A1:A4">
          <cfRule type="colorScale" priority="1">
            <colorScale>
              <cfvo type="num" val="-10"/><cfvo type="percentile" val="60"/><cfvo type="num" val="90"/>
              <color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/>
            </colorScale>
          </cfRule>
          <cfRule type="dataBar" priority="2">
            <dataBar showValue="0" minLength="20" maxLength="80">
              <cfvo type="num" val="-10"/><cfvo type="percent" val="90"/>
              <color rgb="FF5B9BD5"/>
            </dataBar>
          </cfRule>
        </conditionalFormatting>
      </worksheet>
    `);

    expect(diagnoseXlsxConditionalFormatting(worksheet, null)).toEqual([
      expect.objectContaining({
        code: 'xlsx.conditional-formatting',
        severity: 'info',
        message: expect.stringContaining('2 supported'),
      }),
    ]);
  });
});
