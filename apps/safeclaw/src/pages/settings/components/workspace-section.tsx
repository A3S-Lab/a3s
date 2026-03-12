import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import settingsModel from "@/models/settings.model";
import { FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { useSnapshot } from "valtio";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export function WorkspaceSection() {
	const snap = useSnapshot(settingsModel.state);

	const handleChange = (value: string) => {
		settingsModel.setAgentDefaults({ workspaceRoot: value });
	};

	const handlePick = async () => {
		const selected = await openDialog({ directory: true, multiple: false });
		if (typeof selected === "string") handleChange(selected);
	};

	const handleSave = () => {
		toast.success("工作区配置已保存");
	};

	return (
		<div className="space-y-8">
			<div>
				<h2 className="text-base font-semibold mb-1">工作区配置</h2>
				<p className="text-sm text-muted-foreground">
					配置统一的工作区根目录，智能体和会话的工作区将自动在此目录下创建。
				</p>
			</div>

			<div className="space-y-1.5">
				<label htmlFor="workspace-root" className="text-sm font-medium">
					工作区根目录
				</label>
				<div className="flex gap-1.5">
					<Input
						id="workspace-root"
						value={snap.agentDefaults.workspaceRoot}
						onChange={(e) => handleChange(e.target.value)}
						className="h-9 font-mono text-sm"
						placeholder="/path/to/workspace"
					/>
					<Button
						type="button"
						variant="outline"
						size="sm"
						className="h-9 px-2.5 shrink-0"
						title="浏览目录"
						onClick={handlePick}
					>
						<FolderOpen className="size-4" />
					</Button>
				</div>
				<div className="text-xs text-muted-foreground space-y-0.5">
					<p>程序将在此目录下自动创建：</p>
					<p className="font-mono pl-2">
						agents/&lt;agent-id&gt;/ — 智能体默认工作区（含
						skills/、flows/、tasks/、knowledge/）
					</p>
					<p className="font-mono pl-2">
						sessions/&lt;session-folder&gt;/ — 每个会话的独立工作区
					</p>
				</div>
			</div>

			<Button size="sm" onClick={handleSave}>
				保存
			</Button>
		</div>
	);
}
