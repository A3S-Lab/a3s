import { describe, expect, it } from 'vitest';
import { workOfficeAgentInstruction } from './work-office-agent';

describe('workOfficeAgentInstruction', () => {
  it('routes a bound Office artifact through the native Use worker and Skill', () => {
    const instruction = workOfficeAgentInstruction({
      title: '季度计划',
      localPath: 'D:\\workspace\\季度计划.docx',
      instruction: '把摘要改得更简洁。',
    });

    expect(instruction).toContain('D:\\workspace\\季度计划.docx');
    expect(instruction).toContain('专用 use worker');
    expect(instruction).toContain('a3s-office Skill');
    expect(instruction).toContain('use/office 原生 MCP');
    expect(instruction).toContain('保存回同一路径');
    expect(instruction).toContain('不要用 shell');
  });

  it('does not claim disk automation for an unbound library artifact', () => {
    const instruction = workOfficeAgentInstruction({
      title: '草稿',
      instruction: '重写标题。',
    });

    expect(instruction).toContain('尚未绑定到本地 Workspace 文件');
    expect(instruction).toContain('先提醒我将产物保存到本地 Workspace');
  });
});
