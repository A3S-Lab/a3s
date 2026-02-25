import TiptapEditor, {
	type TiptapEditorRef,
} from "@/components/custom/tiptap-editor";
import { cn } from "@/lib/utils";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { sendToSession } from "@/hooks/use-agent-ws";
import { useTts } from "@/hooks/use-tts";
import { agentApi } from "@/lib/agent-api";
import {
	FileText,
	Image,
	Loader2,
	Paperclip,
	Send,
	Upload,
	User,
	X,
} from "lucide-react";
import React, { useCallback, useMemo, useRef, useState } from "react";
import { useSnapshot } from "valtio";
import { SessionStatusBar } from "./session-status-bar";
import { TtsToggle } from "./tts-toggle";

/** A pending attachment with metadata for display */
interface PendingFile {
	id: string;
	name: string;
	media_type: string;
	data: string;
	/** 0–100, undefined once complete */
	progress?: number;
}

let _fileIdCounter = 0;
function nextFileId(): string {
	return `file-${Date.now()}-${++_fileIdCounter}`;
}

export function AgentInput({
	sessionId,
	disabled,
	onSend,
}: {
	sessionId: string;
	disabled: boolean;
	onSend?: (
		text: string,
		images?: { media_type: string; data: string }[],
	) => void;
}) {
	const editorRef = useRef<TiptapEditorRef>(null);
	const fileInputRef = useRef<HTMLInputElement>(null);
	const imageInputRef = useRef<HTMLInputElement>(null);
	const [isEmpty, setIsEmpty] = useState(true);
	const [pendingFiles, setPendingFiles] = useState<PendingFile[]>([]);
	const [isDragging, setIsDragging] = useState(false);
	const dragCounterRef = useRef(0);
	const { sessionStatus, sdkSessions } = useSnapshot(agentModel.state);
	const personaSnap = useSnapshot(personaModel.state);
	const isRunning = sessionStatus[sessionId] === "running";
	const tts = useTts();

	const allFilesReady = pendingFiles.every((f) => f.progress === undefined);

	const mentionItems = useMemo(() => {
		return personaModel
			.getAllPersonas()
			.filter((p) => p.id !== "company-group")
			.map((p) => ({
				id: p.id,
				label: p.name,
				description: p.description,
				group: "智能体",
				icon: <User className="size-3 text-primary" />,
			}));
	}, [personaSnap.serverPersonas, personaSnap.customPersonas]);

	const sessionByPersona = useMemo(() => {
		const map: Record<string, string> = {};
		for (const s of sdkSessions) {
			if (s.archived) continue;
			const pid = personaSnap.sessionPersonas[s.session_id];
			if (!pid) continue;
			if (
				!map[pid] ||
				s.created_at >
					(sdkSessions.find((x) => x.session_id === map[pid])?.created_at ?? 0)
			) {
				map[pid] = s.session_id;
			}
		}
		return map;
	}, [sdkSessions, personaSnap.sessionPersonas]);

	// ── File processing with progress ──

	const processFile = useCallback(
		(file: File) => {
			const id = nextFileId();
			const isImage = file.type.startsWith("image/");

			// Add placeholder with 0% progress
			setPendingFiles((prev) => [
				...prev,
				{
					id,
					name: file.name,
					media_type: isImage ? file.type : "text/plain",
					data: "",
					progress: 0,
				},
			]);

			const reader = new FileReader();
			reader.onprogress = (e) => {
				if (e.lengthComputable) {
					const pct = Math.round((e.loaded / e.total) * 100);
					setPendingFiles((prev) =>
						prev.map((f) => (f.id === id ? { ...f, progress: pct } : f)),
					);
				}
			};
			reader.onload = () => {
				const result = reader.result as string;
				if (isImage) {
					const [header, data] = result.split(",");
					const media_type = header
						.replace("data:", "")
						.replace(";base64", "");
					setPendingFiles((prev) =>
						prev.map((f) =>
							f.id === id
								? { ...f, media_type, data, progress: undefined }
								: f,
						),
					);
				} else {
					// Text file — read as text, then encode
					const textReader = new FileReader();
					textReader.onload = () => {
						const text = textReader.result as string;
						const encoded = btoa(
							unescape(
								encodeURIComponent(`# ${file.name}\n\n${text}`),
							),
						);
						setPendingFiles((prev) =>
							prev.map((f) =>
								f.id === id
									? {
											...f,
											media_type: "text/plain",
											data: encoded,
											progress: undefined,
										}
									: f,
							),
						);
					};
					textReader.onerror = () => {
						setPendingFiles((prev) => prev.filter((f) => f.id !== id));
					};
					textReader.readAsText(file);
				}
			};
			reader.onerror = () => {
				setPendingFiles((prev) => prev.filter((f) => f.id !== id));
			};

			if (isImage) {
				reader.readAsDataURL(file);
			} else {
				// Use readAsDataURL just for progress tracking, then re-read as text
				reader.readAsDataURL(file);
			}
		},
		[],
	);

	const processFiles = useCallback(
		(files: File[]) => {
			for (const file of files) {
				processFile(file);
			}
		},
		[processFile],
	);

	const removeFile = useCallback((id: string) => {
		setPendingFiles((prev) => prev.filter((f) => f.id !== id));
	}, []);

	// ── Handlers ──

	const handleSubmit = useCallback(() => {
		if (!editorRef.current || disabled) return;
		const text = editorRef.current.getText().trim();
		const readyFiles = pendingFiles.filter((f) => f.progress === undefined);
		if (!text && readyFiles.length === 0) return;

		const mentionedIds = editorRef.current.getMentions();
		if (mentionedIds.length > 0) {
			const content = `[来自 Agent] ${text}`;
			for (const personaId of mentionedIds) {
				const targetSid = sessionByPersona[personaId];
				if (targetSid) {
					agentApi
						.sendAgentMessage(targetSid, `mention:${targetSid}`, content)
						.catch(() => {});
				}
			}
		}

		const images = readyFiles.map((f) => ({
			media_type: f.media_type,
			data: f.data,
		}));
		onSend?.(text, images.length > 0 ? images : undefined);
		editorRef.current.clear();
		setIsEmpty(true);
		setPendingFiles([]);
		setTimeout(() => editorRef.current?.focus(), 0);
	}, [disabled, onSend, pendingFiles, sessionByPersona]);

	const handleEditorChange = useCallback((text: string) => {
		setIsEmpty(!text.trim());
	}, []);

	const handlePasteImages = useCallback(
		(images: { media_type: string; data: string }[]) => {
			if (disabled) return;
			const newFiles: PendingFile[] = images.map((img) => ({
				id: nextFileId(),
				name: `粘贴图片`,
				media_type: img.media_type,
				data: img.data,
			}));
			setPendingFiles((prev) => [...prev, ...newFiles]);
		},
		[disabled],
	);

	const handleInterrupt = useCallback(() => {
		sendToSession(sessionId, { type: "interrupt" });
	}, [sessionId]);

	const handleFileChange = useCallback(
		(e: React.ChangeEvent<HTMLInputElement>) => {
			const files = Array.from(e.target.files || []);
			if (files.length > 0) processFiles(files);
			e.target.value = "";
		},
		[processFiles],
	);

	// ── Drag and drop ──

	const handleDragEnter = useCallback(
		(e: React.DragEvent) => {
			e.preventDefault();
			e.stopPropagation();
			dragCounterRef.current++;
			if (e.dataTransfer.types.includes("Files") && !disabled) {
				setIsDragging(true);
			}
		},
		[disabled],
	);

	const handleDragLeave = useCallback((e: React.DragEvent) => {
		e.preventDefault();
		e.stopPropagation();
		dragCounterRef.current--;
		if (dragCounterRef.current === 0) {
			setIsDragging(false);
		}
	}, []);

	const handleDragOver = useCallback((e: React.DragEvent) => {
		e.preventDefault();
		e.stopPropagation();
	}, []);

	const handleDrop = useCallback(
		(e: React.DragEvent) => {
			e.preventDefault();
			e.stopPropagation();
			dragCounterRef.current = 0;
			setIsDragging(false);
			if (disabled) return;
			const files = Array.from(e.dataTransfer.files);
			if (files.length > 0) processFiles(files);
		},
		[disabled, processFiles],
	);

	return (
		<div
			className="flex flex-col h-full relative"
			onDragEnter={handleDragEnter}
			onDragLeave={handleDragLeave}
			onDragOver={handleDragOver}
			onDrop={handleDrop}
		>
			{/* Drag overlay */}
			{isDragging && (
				<div className="absolute inset-0 z-20 flex items-center justify-center bg-primary/[0.04] border-2 border-dashed border-primary/30 rounded-lg backdrop-blur-sm pointer-events-none">
					<div className="flex flex-col items-center gap-2 text-primary">
						<Upload className="size-8 opacity-60" />
						<span className="text-sm font-medium">拖放文件到此处</span>
					</div>
				</div>
			)}

			{/* Pending files preview */}
			{pendingFiles.length > 0 && (
				<div className="flex items-center gap-2 px-3 py-2 border-b flex-wrap">
					{pendingFiles.map((file) => {
						const isImage = file.media_type.startsWith("image/");
						const isLoading = file.progress !== undefined;
						return (
							<div
								key={file.id}
								className="relative group flex items-center gap-2 rounded-lg border bg-muted/30 pl-1.5 pr-2 py-1 max-w-[200px]"
							>
								{/* Thumbnail / icon */}
								<div className="size-8 shrink-0 rounded-md overflow-hidden bg-muted flex items-center justify-center">
									{isLoading ? (
										<Loader2 className="size-4 text-primary animate-spin" />
									) : isImage && file.data ? (
										<img
											src={`data:${file.media_type};base64,${file.data}`}
											alt={file.name}
											className="size-8 object-cover"
										/>
									) : (
										<FileText className="size-4 text-muted-foreground" />
									)}
								</div>
								{/* File name + progress */}
								<div className="flex-1 min-w-0">
									<p className="text-[11px] font-medium truncate leading-tight">
										{file.name}
									</p>
									{isLoading && (
										<div className="mt-0.5 h-1 rounded-full bg-foreground/[0.06] overflow-hidden">
											<div
												className="h-full rounded-full bg-primary/60 transition-all duration-300"
												style={{ width: `${file.progress}%` }}
											/>
										</div>
									)}
									{!isLoading && (
										<p className="text-[9px] text-muted-foreground/50 leading-tight">
											{isImage ? "图片" : "文件"}
										</p>
									)}
								</div>
								{/* Delete button */}
								<button
									type="button"
									className="absolute -top-1.5 -right-1.5 size-6 rounded-full bg-foreground/80 text-background flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity shadow-sm"
									onClick={() => removeFile(file.id)}
									aria-label="移除附件"
								>
									<X className="size-2.5" />
								</button>
							</div>
						);
					})}
				</div>
			)}

			<input
				ref={fileInputRef}
				type="file"
				className="hidden"
				multiple
				accept=".txt,.md,.json,.csv,.ts,.tsx,.js,.jsx,.py,.rs,.go,.java,.c,.cpp,.h,.yaml,.yml,.toml,.xml,.html,.css"
				onChange={handleFileChange}
			/>
			<input
				ref={imageInputRef}
				type="file"
				className="hidden"
				multiple
				accept="image/*"
				onChange={handleFileChange}
			/>

			<div className="flex-1 min-h-0">
				<TiptapEditor
					ref={editorRef}
					placeholder="输入消息，/ 触发技能，@ 派发给 Agent..."
					disabled={disabled}
					mentionItems={mentionItems}
					onSubmit={() => handleSubmit()}
					onChange={handleEditorChange}
					onPasteImages={handlePasteImages}
				/>
			</div>

			{/* WeChat-style bottom toolbar */}
			<div className="flex items-center gap-1 px-2 py-2 shrink-0">
				<button
					type="button"
					className={cn(
						"flex items-center justify-center size-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06] transition-colors",
						disabled && "opacity-40 cursor-not-allowed",
					)}
					title="上传文件"
					aria-label="上传文件"
					onClick={() => fileInputRef.current?.click()}
					disabled={disabled}
				>
					<Paperclip className="size-[18px]" />
				</button>
				<button
					type="button"
					className={cn(
						"flex items-center justify-center size-8 rounded-full text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06] transition-colors",
						disabled && "opacity-40 cursor-not-allowed",
					)}
					title="上传图片"
					aria-label="上传图片"
					onClick={() => imageInputRef.current?.click()}
					disabled={disabled}
				>
					<Image className="size-[18px]" />
				</button>
				<TtsToggle
					ttsEnabled={tts.ttsEnabled}
					isSpeaking={tts.isSpeaking}
					modelsReady={tts.modelsReady}
					isDownloading={tts.isDownloading}
					downloadProgress={tts.downloadProgress}
					disabled={disabled}
					onToggle={() => tts.setTtsEnabled(!tts.ttsEnabled)}
					onStop={tts.stop}
					onDownload={tts.downloadModels}
				/>
				<div className="ml-auto">
					{isRunning ? (
						<button
							type="button"
							className="flex items-center justify-center size-8 rounded-full bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
							onClick={handleInterrupt}
							aria-label="中断"
							title="中断"
						>
							<X className="size-4" />
						</button>
					) : (
						<button
							type="button"
							className={cn(
								"flex items-center justify-center size-8 rounded-full transition-colors",
								(isEmpty && pendingFiles.length === 0) || !allFilesReady || disabled
									? "bg-muted text-muted-foreground cursor-not-allowed"
									: "bg-primary text-primary-foreground hover:bg-primary/90",
							)}
							disabled={
								(isEmpty && pendingFiles.length === 0) ||
								!allFilesReady ||
								disabled
							}
							onClick={handleSubmit}
							aria-label="发送消息"
							title="发送"
						>
							<Send className="size-4" />
						</button>
					)}
				</div>
			</div>

			<SessionStatusBar sessionId={sessionId} />
		</div>
	);
}
