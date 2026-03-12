/**
 * Folder Manager Dialog - Opens agent workspace subfolders in system file manager
 */
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, ExternalLink, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import personaModel from "@/models/persona.model";
import { getPersonaById } from "@/lib/builtin-personas";
import { getOrInitializeAgentWorkspace } from "@/lib/workspace-utils";

interface FolderManagerDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	personaId: string;
	folderType: "skills" | "flows" | "tasks" | "knowledge";
}

const FOLDER_CONFIG = {
	skills: {
		title: "技能管理",
		description: "管理智能体的技能文件",
		subfolder: "skills",
	},
	flows: {
		title: "工作流管理",
		description: "管理智能体的工作流配置",
		subfolder: "flows",
	},
	tasks: {
		title: "定时任务管理",
		description: "管理智能体的定时任务",
		subfolder: "tasks",
	},
	knowledge: {
		title: "知识库管理",
		description: "管理智能体的知识库文件",
		subfolder: "knowledge",
	},
};

export default function FolderManagerDialog({
	open,
	onOpenChange,
	personaId,
	folderType,
}: FolderManagerDialogProps) {
	const [workdir, setWorkdir] = useState<string | null>(null);
	const [initializing, setInitializing] = useState(false);
	const [opening, setOpening] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const config = FOLDER_CONFIG[folderType];

	const allPersonas = personaModel.getAllPersonas();
	const persona =
		allPersonas.find((p) => p.id === personaId) || getPersonaById(personaId);

	// Auto-initialize workspace when dialog opens
	useEffect(() => {
		if (!open) return;
		setError(null);
		setInitializing(true);
		getOrInitializeAgentWorkspace(personaId)
			.then((path) => setWorkdir(path))
			.catch((err) => setError(`初始化工作区失败: ${err}`))
			.finally(() => setInitializing(false));
	}, [open, personaId]);

	const handleOpenFolder = async () => {
		if (!workdir) return;
		setOpening(true);
		setError(null);
		try {
			const folderPath = `${workdir}/${config.subfolder}`;
			await invoke("plugin:shell|open", { path: folderPath });
			setTimeout(() => onOpenChange(false), 500);
		} catch (err) {
			setError(`无法打开文件夹: ${err}`);
		} finally {
			setOpening(false);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>{config.title}</DialogTitle>
					<DialogDescription>{config.description}</DialogDescription>
				</DialogHeader>

				<div className="space-y-4">
					<div className="flex items-center gap-3 p-3 rounded-lg bg-muted/50">
						<FolderOpen className="size-5 text-muted-foreground shrink-0" />
						<div className="flex-1 min-w-0">
							<p className="text-sm font-medium">{persona?.name}</p>
							{initializing ? (
								<p className="text-xs text-muted-foreground flex items-center gap-1">
									<Loader2 className="size-3 animate-spin" />
									正在初始化工作区...
								</p>
							) : workdir ? (
								<p className="text-xs text-muted-foreground truncate">
									{workdir}/{config.subfolder}
								</p>
							) : (
								<p className="text-xs text-destructive">
									未配置全局工作区，请在设置中配置
								</p>
							)}
						</div>
					</div>

					{error && (
						<div className="text-sm text-destructive bg-destructive/10 p-3 rounded-lg">
							{error}
						</div>
					)}

					<p className="text-xs text-muted-foreground">
						智能体的默认工作区，所有会话共享。此处配置的
						{config.title.replace("管理", "")}对该智能体所有会话生效。
					</p>

					<div className="flex justify-end gap-2">
						<Button
							variant="outline"
							onClick={() => onOpenChange(false)}
							disabled={opening}
						>
							取消
						</Button>
						<Button
							onClick={handleOpenFolder}
							disabled={!workdir || initializing || opening}
							className="gap-2"
						>
							{opening ? (
								<>
									<Loader2 className="size-4 animate-spin" />
									打开中...
								</>
							) : (
								<>
									<ExternalLink className="size-4" />
									在文件管理器中打开
								</>
							)}
						</Button>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}
