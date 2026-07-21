import type { Cell, CellMatrix, Sheet } from '@fortune-sheet/core';
import type {
  WorkArtifact,
  WorkArtifactContent,
  WorkArtifactKind,
  WorkPresentationContent,
  WorkSlide,
  WorkTemplate,
} from './work-types';

export const WORK_TEMPLATES: WorkTemplate[] = [
  {
    id: 'blank-document',
    kind: 'document',
    name: '空白文字',
    description: '从一张干净的 A4 页面开始',
    accent: '#2f6fed',
  },
  {
    id: 'project-brief',
    kind: 'document',
    name: '项目方案',
    description: '目标、范围、里程碑与风险',
    accent: '#536de2',
  },
  {
    id: 'blank-spreadsheet',
    kind: 'spreadsheet',
    name: '空白表格',
    description: '公式、筛选与多工作表',
    accent: '#16a36a',
  },
  {
    id: 'quarterly-plan',
    kind: 'spreadsheet',
    name: '季度计划',
    description: '目标进度与预算跟踪',
    accent: '#168f72',
  },
  {
    id: 'blank-presentation',
    kind: 'presentation',
    name: '空白演示',
    description: '16:9 宽屏演示文稿',
    accent: '#e16b3d',
  },
  {
    id: 'strategy-deck',
    kind: 'presentation',
    name: '策略汇报',
    description: '结论先行的三页汇报',
    accent: '#c85637',
  },
];

export function createWorkArtifact(templateId: string): WorkArtifact {
  const template = WORK_TEMPLATES.find((item) => item.id === templateId) ?? WORK_TEMPLATES[0];
  const now = Date.now();
  return {
    id: createWorkId('artifact'),
    kind: template.kind,
    title: initialTitle(template.id, template.kind),
    favorite: false,
    createdAt: now,
    updatedAt: now,
    lastOpenedAt: now,
    revision: 1,
    content: contentForTemplate(template.id),
  };
}

export function createWorkId(prefix: string): string {
  const random =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}-${random}`;
}

function initialTitle(templateId: string, kind: WorkArtifactKind): string {
  const titles: Record<string, string> = {
    'project-brief': '新项目方案',
    'quarterly-plan': '季度执行计划',
    'strategy-deck': '业务策略汇报',
  };
  if (titles[templateId]) return titles[templateId];
  if (kind === 'document') return '无标题文字';
  if (kind === 'spreadsheet') return '无标题表格';
  return '无标题演示';
}

function contentForTemplate(templateId: string): WorkArtifactContent {
  if (templateId === 'project-brief') {
    return {
      type: 'document',
      pageSize: 'a4',
      html: [
        '<h1>新项目方案</h1>',
        '<p><strong>负责人：</strong>项目团队　　<strong>更新日期：</strong>今天</p>',
        '<blockquote><p>用一句话说明这项工作的目标，以及完成后会带来什么变化。</p></blockquote>',
        '<h2>背景与目标</h2>',
        '<p>描述当前情况、核心问题和可衡量的成功标准。</p>',
        '<h2>工作范围</h2>',
        '<ul><li><p>需要完成的关键交付物</p></li><li><p>明确不在本期范围内的事项</p></li></ul>',
        '<h2>里程碑</h2>',
        '<ol><li><p>方案确认</p></li><li><p>执行与评审</p></li><li><p>交付与复盘</p></li></ol>',
        '<h2>风险与决策</h2>',
        '<p>记录尚未解决的问题、依赖和决策负责人。</p>',
      ].join(''),
    };
  }
  if (templateId === 'quarterly-plan') {
    return { type: 'spreadsheet', sheets: quarterlyPlanSheets() };
  }
  if (templateId === 'strategy-deck') {
    return strategyPresentation();
  }
  if (templateId === 'blank-spreadsheet') {
    return { type: 'spreadsheet', sheets: [blankSheet()] };
  }
  if (templateId === 'blank-presentation') {
    return { type: 'presentation', slides: [blankSlide()] };
  }
  return {
    type: 'document',
    pageSize: 'a4',
    html: '<h1></h1><p></p>',
  };
}

function blankSheet(): Sheet {
  return {
    id: createWorkId('sheet'),
    name: '工作表1',
    status: 1,
    order: 0,
    row: 60,
    column: 26,
    data: emptyMatrix(60, 26),
  };
}

function quarterlyPlanSheets(): Sheet[] {
  const data = emptyMatrix(40, 12);
  data[0][0] = styledCell('季度执行计划', { bl: 1, fs: 16, fc: '#ffffff', bg: '#168f72' });
  data[2][0] = headerCell('目标');
  data[2][1] = headerCell('负责人');
  data[2][2] = headerCell('一月');
  data[2][3] = headerCell('二月');
  data[2][4] = headerCell('三月');
  data[2][5] = headerCell('完成率');
  data[2][6] = headerCell('状态');
  const rows: Array<[string, string, number, number, number, string, string]> = [
    ['客户洞察报告', '林岚', 1, 1, 0, '=SUM(C4:E4)/3', '进行中'],
    ['新版发布', '周启', 0.8, 0.6, 0, '=AVERAGE(C5:E5)', '有风险'],
    ['渠道增长', '陈一', 1, 0.9, 0.7, '=AVERAGE(C6:E6)', '正常'],
    ['团队能力建设', '项目组', 1, 1, 1, '=AVERAGE(C7:E7)', '已完成'],
  ];
  rows.forEach((row, rowIndex) => {
    row.forEach((value, columnIndex) => {
      data[rowIndex + 3][columnIndex] = styledCell(value, {
        bg: rowIndex % 2 ? '#f7faf9' : '#ffffff',
      });
    });
  });
  return [
    {
      id: createWorkId('sheet'),
      name: '执行看板',
      status: 1,
      order: 0,
      row: 40,
      column: 12,
      data,
      config: {
        columnlen: { 0: 180, 1: 96, 2: 76, 3: 76, 4: 76, 5: 96, 6: 96 },
        rowlen: { 0: 34, 2: 28 },
        merge: {
          '0_0': { r: 0, c: 0, rs: 1, cs: 7 },
        },
      },
    },
  ];
}

function emptyMatrix(rows: number, columns: number): CellMatrix {
  return Array.from({ length: rows }, () => Array<Cell | null>(columns).fill(null));
}

function styledCell(value: string | number, style: Partial<Cell> = {}): Cell {
  const formula = typeof value === 'string' && value.startsWith('=') ? value : undefined;
  return {
    v: formula ? undefined : value,
    m: formula ? '' : String(value),
    f: formula,
    ...style,
  };
}

function headerCell(value: string): Cell {
  return styledCell(value, {
    bl: 1,
    fc: '#215446',
    bg: '#dff3ec',
    ht: 0,
    vt: 0,
  });
}

function blankSlide(): WorkSlide {
  return {
    id: createWorkId('slide'),
    name: '标题幻灯片',
    background: '#ffffff',
    elements: [
      {
        id: createWorkId('element'),
        type: 'text',
        x: 12,
        y: 25,
        width: 76,
        height: 18,
        text: '单击添加标题',
        fontSize: 34,
        color: '#172033',
        fill: 'transparent',
        bold: true,
        align: 'center',
      },
      {
        id: createWorkId('element'),
        type: 'text',
        x: 18,
        y: 49,
        width: 64,
        height: 10,
        text: '添加副标题',
        fontSize: 17,
        color: '#727b8f',
        fill: 'transparent',
        bold: false,
        align: 'center',
      },
    ],
  };
}

function strategyPresentation(): WorkPresentationContent {
  const slides: WorkSlide[] = [
    {
      id: createWorkId('slide'),
      name: '封面',
      background: '#16213d',
      elements: [
        {
          id: createWorkId('element'),
          type: 'shape',
          x: 8,
          y: 12,
          width: 9,
          height: 3,
          text: '',
          fontSize: 12,
          color: '#ffffff',
          fill: '#ffb15a',
          bold: false,
          align: 'left',
          radius: 2,
        },
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 30,
          width: 72,
          height: 24,
          text: '业务策略汇报',
          fontSize: 38,
          color: '#ffffff',
          fill: 'transparent',
          bold: true,
          align: 'left',
        },
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 58,
          width: 62,
          height: 10,
          text: '把最重要的结论放在标题中',
          fontSize: 17,
          color: '#b8c4df',
          fill: 'transparent',
          bold: false,
          align: 'left',
        },
      ],
    },
    {
      id: createWorkId('slide'),
      name: '核心判断',
      background: '#f7f4ee',
      elements: [
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 10,
          width: 84,
          height: 11,
          text: '01　核心判断',
          fontSize: 15,
          color: '#b44e34',
          fill: 'transparent',
          bold: true,
          align: 'left',
        },
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 27,
          width: 76,
          height: 22,
          text: '用一句可以独立成立的话，说明我们看到了什么。',
          fontSize: 31,
          color: '#20273a',
          fill: 'transparent',
          bold: true,
          align: 'left',
        },
        {
          id: createWorkId('element'),
          type: 'shape',
          x: 8,
          y: 60,
          width: 84,
          height: 22,
          text: '关键证据或数据',
          fontSize: 18,
          color: '#ffffff',
          fill: '#b44e34',
          bold: true,
          align: 'center',
          radius: 3,
        },
      ],
    },
    {
      id: createWorkId('slide'),
      name: '下一步',
      background: '#ffffff',
      elements: [
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 10,
          width: 84,
          height: 12,
          text: '02　下一步',
          fontSize: 15,
          color: '#b44e34',
          fill: 'transparent',
          bold: true,
          align: 'left',
        },
        {
          id: createWorkId('element'),
          type: 'text',
          x: 8,
          y: 28,
          width: 84,
          height: 46,
          text: '1　确认优先级\n2　指定负责人\n3　设定可验证的里程碑',
          fontSize: 26,
          color: '#20273a',
          fill: '#f3f0ea',
          bold: false,
          align: 'left',
          radius: 3,
        },
      ],
    },
  ];
  return { type: 'presentation', slides };
}
