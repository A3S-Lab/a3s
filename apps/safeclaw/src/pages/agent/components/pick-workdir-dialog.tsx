import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { agentApi } from "@/lib/agent-api";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import settingsModel, {
	getPreferredSessionModel,
	resolveApiKey,
	resolveBaseUrl,
} from "@/models/settings.model";
import { connectSession } from "@/hooks/use-agent-ws";
import { AlertCircle, FolderOpen, Loader2 } from "lucide-react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { useCallback, useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";

/** Generate a session subfolder name: <persona-id>-YYYYMMDD-HHmmss */
function sessionFolderName(personaId: string): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, "0");
	const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
	const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
	return `${personaId}-${date}-${time}`;
}

function joinPath(...parts: string[]): string {
	return parts
		.map((p, i) =>
			i === 0 ? p.replace(/\/+$/, "") : p.replace(/^\/+|\/+$/g, ""),
		)
		.filter(Boolean)
		.join("/");
}

interface PickWorkdirDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	personaId: string | null;
	onCreated: (sessionId: string) => void;
}

export default function PickWorkdirDialog({
	open,
	onOpenChange,
	personaId,
	onCreated,
}: PickWorkdirDialogProps) {
	const navigate = useNavigate();
	const [cwd, setCwd] = useState("");
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const defaultCwd = settingsModel.state.agentDefaults.defaultCwd.trim();
	const hasDefaultCwd = defaultCwd.length > 0;

	const persona = useMemo(
		() =>
			personaId
				? personaModel.getAllPersonas().find((p) => p.id === personaId)
				: null,
		[personaId],
	);

	const avatarConfig = useMemo(
		() => (persona ? genConfig(persona.avatar) : null),
		[persona],
	);

	// Compute a fresh suggested path each time the dialog opens
	useEffect(() => {
		if (open && personaId && hasDefaultCwd) {
			setCwd(joinPath(defaultCwd, sessionFolderName(personaId)));
		} else if (open) {
			setCwd("");
		}
		setError(null);
	}, [open, personaId, hasDefaultCwd, defaultCwd]);

	const handlePickDir = useCallback(async () => {
		const selected = await openDialog({ directory: true, multiple: false });
		if (typeof selected === "string") setCwd(selected);
	}, []);

	const handleCreate = useCallback(async () => {
		if (!personaId || !persona) return;

		const trimmedCwd = cwd.trim();
		if (!trimmedCwd) {
			setError("工作区目录不能为空");
			return;
		}

		setLoading(true);
		setError(null);
		try {
			// Initialize workspace: create directory, agents/, skills/, A3sfile
			await invoke("init_workspace", { path: trimmedCwd });

			const preferred = getPreferredSessionModel();
			const modelId = persona.defaultModel || preferred.modelId || undefined;
			const providerName = persona.defaultModel?.includes("/")
				? persona.defaultModel.split("/")[0]
				: preferred.providerName;
			const apiKey = modelId
				? resolveApiKey(providerName, modelId.split("/").pop() || modelId)
				: "";
			const baseUrl = modelId
				? resolveBaseUrl(providerName, modelId.split("/").pop() || modelId)
				: "";

			const result = await agentApi.createSession({
				persona_id: personaId,
				model: modelId,
				permission_mode: persona.defaultPermissionMode || "default",
				cwd: trimmedCwd,
				system_prompt: persona.systemPrompt || undefined,
				api_key: apiKey || undefined,
				base_url: baseUrl || undefined,
			});

			if (result?.error) {
				setError(result.error);
				return;
			}

			const sid = result.session_id;
			personaModel.setSessionPersona(sid, personaId);
			agentModel.setMessages(sid, []);

			const updated = await agentApi.listSessions();
			if (Array.isArray(updated)) agentModel.setSdkSessions(updated);

			connectSession(sid);
			agentModel.setCurrentSession(sid);
			agentModel.clearUnread(sid);

			onCreated(sid);
			onOpenChange(false);
		} catch (e) {
			setError(
				e instanceof Error
					? e.message
					: "无法连接到网关，请检查 SafeClaw 是否正在运行",
			);
		} finally {
			setLoading(false);
		}
	}, [personaId, persona, cwd, onCreated, onOpenChange]);

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-[420px]">
				<DialogHeader>
					<DialogTitle className="flex items-center gap-2.5 text-base">
						{avatarConfig && (
							<NiceAvatar className="size-7 shrink-0" {...avatarConfig} />
						)}
						{persona?.name ?? "新建会话"}
					</DialogTitle>
				</DialogHeader>

				<div className="py-1 space-y-4">
					{!hasDefaultCwd ? (
						/* ── No default workspace configured ── */
						<div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3 space-y-2">
							<div className="flex items-center gap-2 text-amber-600 dark:text-amber-400">
								<AlertCircle className="size-4 shrink-0" />
								<span className="text-sm font-medium">未配置默认工作区</span>
							</div>
							<p className="text-xs text-muted-foreground">
								创建会话前需要先在设置中配置默认工作区根目录，每个会话将在其下自动创建独立文件夹。
							</p>
							<Button
								size="sm"
								variant="outline"
								className="h-7 text-xs"
								onClick={() => {
									onOpenChange(false);
									navigate("/settings");
								}}
							>
								前往设置
							</Button>
						</div>
					) : (
						/* ── Working directory picker ── */
						<>
							<p className="text-sm text-muted-foreground">
								工作区目录已自动生成，Agent
								将在此目录中运行。你也可以手动修改或重新选择。
							</p>

							<div className="space-y-1.5">
								<Label
									htmlFor="workdir"
									className="text-xs flex items-center gap-1"
								>
									<FolderOpen className="size-3" />
									工作区目录
								</Label>
								<div className="flex gap-1.5">
									<Input
										id="workdir"
										value={cwd}
										onChange={(e) => setCwd(e.target.value)}
										className="h-8 text-xs font-mono flex-1"
										placeholder={joinPath(defaultCwd, "session-folder")}
									/>
									<Button
										type="button"
										variant="outline"
										size="sm"
										className="h-8 px-2.5 shrink-0"
										onClick={handlePickDir}
										title="浏览目录"
									>
										<FolderOpen className="size-3.5" />
									</Button>
								</div>
							</div>
						</>
					)}

					{error && (
						<p className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
							{error}
						</p>
					)}
				</div>

				<DialogFooter>
					<Button
						variant="outline"
						size="sm"
						onClick={() => onOpenChange(false)}
						disabled={loading}
					>
						取消
					</Button>
					<Button
						size="sm"
						onClick={handleCreate}
						disabled={loading || !hasDefaultCwd}
					>
						{loading ? (
							<>
								<Loader2 className="size-3.5 animate-spin mr-1.5" />
								创建中...
							</>
						) : (
							"开始会话"
						)}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
