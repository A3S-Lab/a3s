/**
 * Skills Panel - Manage agent skills
 */
import { useState, useEffect } from "react";
import { Plus, FileCode, Trash2, FolderOpen } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getAgentWorkspacePath } from "@/lib/workspace-utils";
import { toast } from "sonner";

interface Skill {
	name: string;
	path: string;
}

export default function SkillsPanel({ agentId }: { agentId: string }) {
	const [skills, setSkills] = useState<Skill[]>([]);
	const [loading, setLoading] = useState(true);

	useEffect(() => {
		loadSkills();
	}, [agentId]);

	const loadSkills = async () => {
		setLoading(true);
		try {
			const workspacePath = await getAgentWorkspacePath(agentId);
			const skillsPath = `${workspacePath}/skills`;

			// Read skills directory
			try {
				const entries = await invoke<string[]>("plugin:fs|read_dir", {
					path: skillsPath,
				});

				const skillFiles = entries
					.filter((name) => name.endsWith(".py") || name.endsWith(".js"))
					.map((name) => ({
						name,
						path: `${skillsPath}/${name}`,
					}));

				setSkills(skillFiles);
			} catch (err) {
				// Directory might not exist yet
				setSkills([]);
			}
		} catch (err) {
			console.error("Failed to load skills:", err);
			toast.error("加载技能失败");
		} finally {
			setLoading(false);
		}
	};

	const handleOpenFolder = async () => {
		try {
			const workspacePath = await getAgentWorkspacePath(agentId);
			const skillsPath = `${workspacePath}/skills`;
			await invoke("plugin:shell|open", { path: skillsPath });
		} catch (err) {
			toast.error("无法打开文件夹");
		}
	};

	const handleDeleteSkill = async (skill: Skill) => {
		if (!confirm(`确定要删除技能 "${skill.name}" 吗？`)) return;

		try {
			await invoke("plugin:fs|remove", { path: skill.path });
			toast.success("技能已删除");
			loadSkills();
		} catch (err) {
			toast.error("删除失败");
		}
	};

	return (
		<div className="flex flex-col h-full">
			{/* Header */}
			<div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
				<div>
					<p className="text-xs text-muted-foreground">
						{skills.length} 个技能文件
					</p>
				</div>
				<button
					type="button"
					onClick={handleOpenFolder}
					className="flex items-center gap-1.5 px-2 py-1 text-xs rounded-lg border hover:bg-muted transition-colors"
					title="在文件管理器中打开"
				>
					<FolderOpen className="size-3" />
					打开文件夹
				</button>
			</div>

			{/* List */}
			<div className="flex-1 overflow-y-auto p-3">
				{loading ? (
					<div className="flex items-center justify-center h-full text-muted-foreground text-sm">
						加载中…
					</div>
				) : skills.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
						<FileCode className="size-10 opacity-20" />
						<div className="text-center">
							<p className="text-xs">还没有技能文件</p>
							<p className="text-xs mt-1">
								点击"打开文件夹"添加 .py 或 .js 技能文件
							</p>
						</div>
					</div>
				) : (
					<div className="flex flex-col gap-2">
						{skills.map((skill) => (
							<div
								key={skill.path}
								className="group flex items-center justify-between p-3 rounded-lg border bg-card hover:border-primary/40 hover:shadow-sm transition-all"
							>
								<div className="flex items-center gap-2 min-w-0">
									<div className="flex items-center justify-center size-7 rounded-lg bg-primary/10 shrink-0">
										<FileCode className="size-3.5 text-primary" />
									</div>
									<span className="text-sm font-medium truncate font-mono">
										{skill.name}
									</span>
								</div>
								<button
									type="button"
									onClick={() => handleDeleteSkill(skill)}
									className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-all opacity-0 group-hover:opacity-100"
									title="删除"
								>
									<Trash2 className="size-3" />
								</button>
							</div>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
