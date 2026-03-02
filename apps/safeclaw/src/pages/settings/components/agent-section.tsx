import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import settingsModel from "@/models/settings.model";
import { useState } from "react";
import { toast } from "sonner";
import { useSnapshot } from "valtio";

const SENSITIVE_TOOL_PRESETS: Array<{
	key: string;
	label: string;
	tools: string[];
	description: string;
}> = [
	{
		key: "dev",
		label: "开发机",
		tools: ["bash", "shell", "git", "write", "edit", "delete", "network*"],
		description: "日常开发常见高风险工具",
	},
	{
		key: "prod",
		label: "生产机",
		tools: ["bash", "shell", "rm", "delete", "write", "network*", "mcp*"],
		description: "偏保守，覆盖写入与外联路径",
	},
	{
		key: "strict",
		label: "严格模式",
		tools: ["*"],
		description: "所有权限请求都走本地隐私模型",
	},
];

export function AgentSection() {
	const snap = useSnapshot(settingsModel.state);
	const d = snap.agentDefaults;
	const [maxTurns, setMaxTurns] = useState(String(d.maxTurns));
	const [defaultCwd, setDefaultCwd] = useState(d.defaultCwd);
	const [autoArchiveHours, setAutoArchiveHours] = useState(
		String(d.autoArchiveHours),
	);
	const [sensitiveTools, setSensitiveTools] = useState(
		(d.sensitiveTools || []).join("\n"),
	);
	const parsedSensitiveTools = sensitiveTools
		.split(/[,\n]/)
		.map((v) => v.trim())
		.filter(Boolean);

	const handleSave = () => {
		const turns = Number.parseInt(maxTurns, 10);
		const archive = Number.parseInt(autoArchiveHours, 10);
		const uniqueSensitiveTools = Array.from(new Set(parsedSensitiveTools));
		settingsModel.setAgentDefaults({
			maxTurns: Number.isNaN(turns) || turns < 0 ? 0 : turns,
			defaultCwd: defaultCwd.trim(),
			autoArchiveHours: Number.isNaN(archive) || archive < 0 ? 0 : archive,
			sensitiveTools: uniqueSensitiveTools,
		});
		toast.success("Agent 配置已保存");
	};

	const applySensitiveToolPreset = (tools: string[]) => {
		setSensitiveTools(tools.join("\n"));
	};

	return (
		<div className="space-y-8">
			<div>
				<h2 className="text-base font-semibold mb-1">Agent 配置</h2>
				<p className="text-sm text-muted-foreground">
					新建会话时使用的全局默认值。
				</p>
			</div>

			<div className="space-y-5">
				<div className="space-y-1.5">
					<label htmlFor="agent-max-turns" className="text-sm font-medium">
						最大轮次
					</label>
					<Input
						id="agent-max-turns"
						type="number"
						min={0}
						value={maxTurns}
						onChange={(e) => setMaxTurns(e.target.value)}
						className="h-9 w-40"
						placeholder="0"
					/>
					<p className="text-xs text-muted-foreground">
						每个会话允许的最大对话轮次。0 表示不限制。
					</p>
				</div>

				<div className="space-y-1.5">
					<label htmlFor="agent-default-cwd" className="text-sm font-medium">
						默认工作目录
					</label>
					<Input
						id="agent-default-cwd"
						value={defaultCwd}
						onChange={(e) => setDefaultCwd(e.target.value)}
						className="h-9 font-mono text-sm"
						placeholder="/path/to/workspace"
					/>
					<p className="text-xs text-muted-foreground">
						新建会话时的默认工作目录。留空则使用进程当前目录。
					</p>
				</div>

				<div className="space-y-1.5">
					<label
						htmlFor="agent-auto-archive-hours"
						className="text-sm font-medium"
					>
						自动归档（小时）
					</label>
					<Input
						id="agent-auto-archive-hours"
						type="number"
						min={0}
						value={autoArchiveHours}
						onChange={(e) => setAutoArchiveHours(e.target.value)}
						className="h-9 w-40"
						placeholder="0"
					/>
					<p className="text-xs text-muted-foreground">
						空闲超过指定小时数后自动归档会话。0 表示不自动归档。
					</p>
				</div>

				<div className="space-y-1.5">
					<label
						htmlFor="agent-sensitive-tools"
						className="text-sm font-medium"
					>
						敏感工具列表
					</label>
					<Textarea
						id="agent-sensitive-tools"
						value={sensitiveTools}
						onChange={(e) => setSensitiveTools(e.target.value)}
						className="min-h-28 font-mono text-xs"
						placeholder={"bash\nwrite\nnetwork*\nmcp*"}
					/>
					<div className="flex flex-wrap gap-2 pt-1">
						{SENSITIVE_TOOL_PRESETS.map((preset) => (
							<Button
								key={preset.key}
								type="button"
								variant="outline"
								size="sm"
								className="h-7 text-xs"
								onClick={() => applySensitiveToolPreset(preset.tools)}
								title={preset.description}
							>
								{preset.label}
							</Button>
						))}
					</div>
					<p className="text-xs text-muted-foreground">
						当权限请求命中这些工具名时，会话将自动切换到本地隐私模型。
						支持每行一个或逗号分隔，`*` 表示前缀匹配。
					</p>
					<p className="text-xs text-muted-foreground">
						当前规则数：{parsedSensitiveTools.length}
					</p>
				</div>
			</div>

			<Button size="sm" onClick={handleSave}>
				保存
			</Button>
		</div>
	);
}
