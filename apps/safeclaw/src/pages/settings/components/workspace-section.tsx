import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import settingsModel from "@/models/settings.model";
import { FolderOpen } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useSnapshot } from "valtio";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export function WorkspaceSection() {
	const snap = useSnapshot(settingsModel.state);
	const [defaultCwd, setDefaultCwd] = useState(snap.agentDefaults.defaultCwd);

	const handleSave = () => {
		settingsModel.setAgentDefaults({ defaultCwd: defaultCwd.trim() });
		toast.success("工作区配置已保存");
	};

	const handlePickDir = async () => {
		const selected = await openDialog({ directory: true, multiple: false });
		if (typeof selected === "string") setDefaultCwd(selected);
	};

	return (
		<div className="space-y-8">
			<div>
				<h2 className="text-base font-semibold mb-1">工作区配置</h2>
				<p className="text-sm text-muted-foreground">
					配置 Agent
					会话的默认工作区目录。每个新会话将在此目录下自动创建独立子文件夹。
				</p>
			</div>

			<div className="space-y-5">
				<div className="space-y-1.5">
					<label
						htmlFor="workspace-default-cwd"
						className="text-sm font-medium"
					>
						默认工作区根目录
					</label>
					<div className="flex gap-1.5">
						<Input
							id="workspace-default-cwd"
							value={defaultCwd}
							onChange={(e) => setDefaultCwd(e.target.value)}
							className="h-9 font-mono text-sm"
							placeholder="/path/to/workspace"
						/>
						<Button
							type="button"
							variant="outline"
							size="sm"
							className="h-9 px-2.5 shrink-0"
							title="浏览目录"
							onClick={handlePickDir}
						>
							<FolderOpen className="size-4" />
						</Button>
					</div>
					<p className="text-xs text-muted-foreground">
						每个新会话会在此目录下自动创建独立子文件夹作为工作区。必须先配置此项才能创建会话。
					</p>
				</div>
			</div>

			<Button size="sm" onClick={handleSave}>
				保存
			</Button>
		</div>
	);
}
