import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import settingsModel from "@/models/settings.model";
import { useState } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

export function AgentSection() {
	const snap = useSnapshot(settingsModel.state);
	const d = snap.agentDefaults;
	const [maxTurns, setMaxTurns] = useState(String(d.maxTurns));
	const [defaultCwd, setDefaultCwd] = useState(d.defaultCwd);
	const [autoArchiveHours, setAutoArchiveHours] = useState(
		String(d.autoArchiveHours),
	);

	const handleSave = () => {
		const turns = parseInt(maxTurns, 10);
		const archive = parseInt(autoArchiveHours, 10);
		settingsModel.setAgentDefaults({
			maxTurns: isNaN(turns) || turns < 0 ? 0 : turns,
			defaultCwd: defaultCwd.trim(),
			autoArchiveHours: isNaN(archive) || archive < 0 ? 0 : archive,
		});
		toast.success("Agent 配置已保存");
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
					<label className="text-sm font-medium">最大轮次</label>
					<Input
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
					<label className="text-sm font-medium">默认工作目录</label>
					<Input
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
					<label className="text-sm font-medium">自动归档（小时）</label>
					<Input
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
			</div>

			<Button size="sm" onClick={handleSave}>
				保存
			</Button>
		</div>
	);
}
