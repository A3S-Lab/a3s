export function workOfficeAgentInstruction({
  title,
  instruction,
  localPath,
}: {
  title: string;
  instruction: string;
  localPath?: string | null;
}): string {
  const subject = `关于当前正在编辑的“${title}”`;
  if (!localPath) {
    return `${subject}：\n${instruction}\n\n当前产物尚未绑定到本地 Workspace 文件。不要声称已通过 Office CLI 修改磁盘文件；需要直接修改时，请先提醒我将产物保存到本地 Workspace。`;
  }

  return `${subject}（本地文件：${localPath}）：\n${instruction}\n\n这是 A3S Web 的人机双写 Office 产物。检查或修改整份文件时，将操作委派给专用 use worker，并严格遵循已内置的 a3s-office Skill：优先使用 use/office 原生 MCP，先验证和读取，再做有边界的批量修改，保存回同一路径，最后复核并报告实际改动。不要用 shell 代替 Office 能力，也不要在结果未知时自动重试写操作。`;
}
