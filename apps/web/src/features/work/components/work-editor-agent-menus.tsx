import { BarChart3, Copy, FileText, ListChecks, MessageSquareText, Sparkles, WandSparkles } from 'lucide-react';
import { showToast } from '../../../state/app-state';
import type { WorkspaceContextMenuItem } from '../../workspace/components/workspace-context-menu';
import {
  createWorkAgentProposalRequest,
  type WorkAgentProposalRequest,
  type WorkAgentProposalTarget,
} from '../work-agent-proposal';
import type { WorkEditorAgentRequest } from '../work-agent-request';
import type { WorkSpreadsheetAgentSelection } from '../work-spreadsheet-agent-context';

type WorkEditorAgentHandler = (request: WorkEditorAgentRequest) => void | Promise<void>;

export function spreadsheetAgentMenuItems(
  selection: WorkSpreadsheetAgentSelection,
  onAgentRequest: WorkEditorAgentHandler,
  applyProposal?: WorkAgentProposalRequest['apply']
): WorkspaceContextMenuItem[] {
  return [
    {
      id: 'copy',
      label: `复制选区 ${selection.reference}`,
      icon: <Copy size={14} />,
      onSelect: () => void copyEditorSelection(selection.clipboard, '表格选区已复制'),
    },
    {
      id: 'ask',
      label: '询问 AI 助手',
      icon: <MessageSquareText size={14} />,
      separatorBefore: true,
      onSelect: () =>
        void onAgentRequest({
          instruction: '请围绕这个表格选区回答我的问题：\n\n问题：',
          selection: selection.context,
        }),
    },
    {
      id: 'analyze',
      label: '分析数据与异常',
      icon: <ListChecks size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请分析这个表格选区，概括关键趋势、异常值、缺失值和可能的数据质量问题。明确区分事实、推断与需要补充的信息。',
          selection: selection.context,
        }),
    },
    {
      id: 'formula',
      label: '解释公式与错误',
      icon: <FileText size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请检查这个表格选区中的公式和值，解释计算逻辑并指出潜在错误、循环引用或不一致。只提供诊断，不要直接修改工作簿。',
          selection: selection.context,
        }),
    },
    {
      id: 'chart',
      label: '建议图表与数据叙事',
      icon: <BarChart3 size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请根据这个表格选区建议最合适的图表类型、分类范围、数据系列、标题和需要突出表达的结论。可在柱形图、条形图、折线图、饼图、圆环图、面积图和雷达图中选择，并解释选择理由；先提供可审阅方案，不要直接修改工作簿。',
          selection: selection.context,
        }),
    },
    {
      id: 'propose',
      label: '建议公式或整理方案',
      icon: <WandSparkles size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请为这个表格选区提出公式、格式或数据整理建议。用 Markdown 表格列出“单元格或范围 / 当前内容 / 建议内容 / 理由”，形成可审阅的差异清单；不要直接修改工作簿。',
          selection: selection.context,
          ...(applyProposal && selection.proposalTargets.length
            ? {
                proposal: createWorkAgentProposalRequest({
                  title: '审阅表格修改',
                  description: `${selection.sheetName}!${selection.reference} · ${selection.proposalTargets.length} 个可修改单元格`,
                  targets: selection.proposalTargets,
                  apply: applyProposal,
                }),
              }
            : {}),
        }),
    },
  ];
}

export function presentationAgentMenuItems(
  selection: string,
  target: 'slide' | 'element',
  onAgentRequest: WorkEditorAgentHandler,
  proposalOptions?: {
    rewriteTargets: readonly WorkAgentProposalTarget[];
    notesTarget: WorkAgentProposalTarget;
    apply: WorkAgentProposalRequest['apply'];
  }
): WorkspaceContextMenuItem[] {
  return [
    {
      id: 'copy',
      label: target === 'element' ? '复制元素内容摘要' : '复制幻灯片内容摘要',
      icon: <Copy size={14} />,
      onSelect: () => void copyEditorSelection(selection, '演示内容摘要已复制'),
    },
    {
      id: 'ask',
      label: '询问 AI 助手',
      icon: <MessageSquareText size={14} />,
      separatorBefore: true,
      onSelect: () =>
        void onAgentRequest({
          instruction: `请围绕这${target === 'element' ? '个选中元素' : '张幻灯片'}回答我的问题：\n\n问题：`,
          selection,
        }),
    },
    {
      id: 'summarize',
      label: target === 'element' ? '总结元素信息' : '总结这张幻灯片',
      icon: <FileText size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction: `请总结这${target === 'element' ? '个演示元素' : '张幻灯片'}传达的核心信息、关键证据和行动项。`,
          selection,
        }),
    },
    {
      id: 'rewrite',
      label: '改进文案与叙事',
      icon: <Sparkles size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请改进选中演示内容的文案、信息层级与叙事节奏。用“原内容 / 建议内容 / 理由”给出可审阅的差异清单；不要直接修改演示文稿。',
          selection,
          ...(proposalOptions?.rewriteTargets.length
            ? {
                proposal: createWorkAgentProposalRequest({
                  title: '审阅演示文案修改',
                  description:
                    target === 'element'
                      ? `${proposalOptions.rewriteTargets.length} 个元素内容目标`
                      : `${proposalOptions.rewriteTargets.length} 个幻灯片内容目标`,
                  targets: proposalOptions.rewriteTargets,
                  apply: proposalOptions.apply,
                }),
              }
            : {}),
        }),
    },
    {
      id: 'design',
      label: '建议版式与视觉层级',
      icon: <WandSparkles size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请检查选中演示内容的版式、对齐、密度、颜色和视觉层级，给出具体可执行的设计建议。先提供方案，不要直接移动、删除或改写元素。',
          selection,
        }),
    },
    {
      id: 'notes',
      label: '起草演讲者备注',
      icon: <FileText size={14} />,
      onSelect: () =>
        void onAgentRequest({
          instruction:
            '请根据选中的演示内容起草简洁自然的演讲者备注，补足过渡语、需要强调的证据和可能被问到的问题。只提供草稿，不要直接修改备注。',
          selection,
          ...(proposalOptions
            ? {
                proposal: createWorkAgentProposalRequest({
                  title: '审阅演讲者备注',
                  description: proposalOptions.notesTarget.label,
                  targets: [proposalOptions.notesTarget],
                  apply: proposalOptions.apply,
                }),
              }
            : {}),
        }),
    },
  ];
}

async function copyEditorSelection(value: string, successMessage: string): Promise<void> {
  try {
    if (!navigator.clipboard?.writeText) throw new Error('Clipboard API is unavailable');
    await navigator.clipboard.writeText(value);
    showToast(successMessage, 'success');
  } catch {
    showToast('无法访问剪贴板，请使用系统复制快捷键。', 'error');
  }
}
