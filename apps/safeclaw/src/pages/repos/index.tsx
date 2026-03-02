import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import repoModel, { type Repo } from "@/models/repo.model";
import { open } from "@tauri-apps/plugin-dialog";
import {
	FolderOpen,
	GitBranch,
	Pin,
	PinOff,
	Plus,
	Search,
	Trash2,
	Code2,
} from "lucide-react";
import { useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

// ---------------------------------------------------------------------------
// Language badge color
// ---------------------------------------------------------------------------

const LANG_COLORS: Record<string, string> = {
	Rust: "bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20",
	TypeScript:
		"bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20",
	JavaScript:
		"bg-yellow-500/10 text-yellow-600 dark:text-yellow-400 border-yellow-500/20",
	Python:
		"bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20",
	Go: "bg-cyan-500/10 text-cyan-600 dark:text-cyan-400 border-cyan-500/20",
	Java: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20",
	"C++":
		"bg-purple-500/10 text-purple-600 dark:text-purple-400 border-purple-500/20",
	C: "bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
};

function langColor(lang: string) {
	return LANG_COLORS[lang] ?? "bg-muted/40 text-muted-foreground border-border";
}

// ---------------------------------------------------------------------------
// Add repo dialog (inline)
// ---------------------------------------------------------------------------

function AddRepoForm({ onDone }: { onDone: () => void }) {
	const [path, setPath] = useState("");
	const [name, setName] = useState("");
	const [description, setDescription] = useState("");

	const handlePickDir = async () => {
		const selected = await open({ directory: true, multiple: false });
		if (typeof selected === "string") {
			setPath(selected);
			if (!name) setName(selected.split("/").pop() ?? "");
		}
	};

	const handleAdd = () => {
		if (!path.trim()) {
			toast.error("请选择工作区目录");
			return;
		}
		repoModel.add({
			name: name.trim() || path.split("/").pop() || path,
			path: path.trim(),
			description: description.trim(),
			language: "",
			pinned: false,
		});
		toast.success("工作区已添加");
		onDone();
	};

	return (
		<div className="rounded-lg border bg-card p-4 space-y-3">
			<h3 className="text-sm font-semibold">添加工作区</h3>

			<div className="space-y-1.5">
				<label className="text-xs text-muted-foreground">目录路径</label>
				<div className="flex gap-1.5">
					<Input
						value={path}
						readOnly
						className="h-8 text-xs font-mono flex-1 cursor-default"
						placeholder="/path/to/repo"
					/>
					<button
						type="button"
						className="flex items-center justify-center size-8 rounded-md border text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors shrink-0"
						onClick={handlePickDir}
						aria-label="选择目录"
					>
						<FolderOpen className="size-3.5" />
					</button>
				</div>
			</div>

			<div className="space-y-1.5">
				<label className="text-xs text-muted-foreground">名称</label>
				<Input
					value={name}
					onChange={(e) => setName(e.target.value)}
					className="h-8 text-xs"
					placeholder="my-project"
				/>
			</div>

			<div className="space-y-1.5">
				<label className="text-xs text-muted-foreground">描述（可选）</label>
				<Input
					value={description}
					onChange={(e) => setDescription(e.target.value)}
					className="h-8 text-xs"
					placeholder="简短描述..."
				/>
			</div>

			<div className="flex gap-2 pt-1">
				<Button size="sm" className="h-7 text-xs" onClick={handleAdd}>
					添加
				</Button>
				<Button
					size="sm"
					variant="ghost"
					className="h-7 text-xs"
					onClick={onDone}
				>
					取消
				</Button>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Repo card
// ---------------------------------------------------------------------------

function RepoCard({
	repo,
	onOpenInAgent,
	onOpenEditor,
}: {
	repo: Repo;
	onOpenInAgent: (repo: Repo) => void;
	onOpenEditor: (repo: Repo) => void;
}) {
	const [confirming, setConfirming] = useState(false);

	return (
		<div
			className={cn(
				"rounded-lg border bg-card transition-all hover:border-foreground/20 cursor-pointer",
				repo.pinned && "border-primary/30",
			)}
			onClick={() => onOpenEditor(repo)}
		>
			<div className="flex items-start gap-3 px-4 py-3">
				<div className="mt-0.5 shrink-0 text-muted-foreground/50">
					<GitBranch className="size-4" />
				</div>

				<div className="flex-1 min-w-0">
					<div className="flex items-center gap-2 flex-wrap">
						<span className="text-sm font-semibold truncate">{repo.name}</span>
						{repo.pinned && <Pin className="size-3 text-primary shrink-0" />}
						{repo.language && (
							<span
								className={cn(
									"text-[10px] rounded border px-1.5 py-0.5 font-medium",
									langColor(repo.language),
								)}
							>
								{repo.language}
							</span>
						)}
					</div>

					<p className="text-[11px] font-mono text-muted-foreground/60 truncate mt-0.5">
						{repo.path}
					</p>

					{repo.description && (
						<p className="text-[12px] text-muted-foreground mt-1 line-clamp-2">
							{repo.description}
						</p>
					)}

					<div className="flex items-center gap-3 mt-1.5 text-[11px] text-muted-foreground/50">
						<span>添加于 {timeAgo(repo.addedAt)}</span>
						{repo.lastOpenedAt && (
							<span>上次打开 {timeAgo(repo.lastOpenedAt)}</span>
						)}
					</div>
				</div>

				<div
					className="flex items-center gap-1 shrink-0"
					onClick={(e) => e.stopPropagation()}
				>
					<button
						type="button"
						className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
						aria-label="在智能体中打开"
						title="在智能体中打开"
						onClick={() => onOpenInAgent(repo)}
					>
						<Code2 className="size-3.5" />
					</button>
					<button
						type="button"
						className={cn(
							"flex items-center justify-center size-7 rounded-md transition-colors",
							repo.pinned
								? "text-primary hover:text-primary/70 hover:bg-foreground/[0.04]"
								: "text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04]",
						)}
						aria-label={repo.pinned ? "取消置顶" : "置顶"}
						title={repo.pinned ? "取消置顶" : "置顶"}
						onClick={() => repoModel.togglePin(repo.id)}
					>
						{repo.pinned ? (
							<PinOff className="size-3.5" />
						) : (
							<Pin className="size-3.5" />
						)}
					</button>
					{confirming ? (
						<div className="flex items-center gap-1">
							<button
								type="button"
								className="text-destructive hover:text-destructive/80 text-[11px] font-medium px-1"
								onClick={() => {
									repoModel.remove(repo.id);
									toast.success("已删除");
								}}
							>
								删除
							</button>
							<button
								type="button"
								className="text-muted-foreground hover:text-foreground text-[11px] px-1"
								onClick={() => setConfirming(false)}
							>
								取消
							</button>
						</div>
					) : (
						<button
							type="button"
							className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-destructive hover:bg-foreground/[0.04] transition-colors"
							aria-label="删除工作区"
							onClick={() => setConfirming(true)}
						>
							<Trash2 className="size-3.5" />
						</button>
					)}
				</div>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export default function ReposPage() {
	const { repos } = useSnapshot(repoModel.state);
	const navigate = useNavigate();
	const [query, setQuery] = useState("");
	const [adding, setAdding] = useState(false);

	const filtered = useMemo(() => {
		const q = query.trim().toLowerCase();
		const list = [...repos].sort((a, b) => {
			if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
			return b.addedAt - a.addedAt;
		});
		if (!q) return list;
		return list.filter(
			(r) =>
				r.name.toLowerCase().includes(q) ||
				r.path.toLowerCase().includes(q) ||
				r.description.toLowerCase().includes(q) ||
				r.language.toLowerCase().includes(q),
		);
	}, [repos, query]);

	const handleOpenInAgent = (repo: Repo) => {
		repoModel.markOpened(repo.id);
		navigate(`/repos/${repo.id}?copilot=1`);
	};

	const handleOpenEditor = (repo: Repo) => {
		navigate(`/repos/${repo.id}`);
	};

	return (
		<div className="flex flex-col h-full px-5 py-4 space-y-4 overflow-y-auto">
			{/* Header */}
			<div className="flex items-center justify-between shrink-0">
				<div>
					<h1 className="text-sm font-bold">工作区</h1>
					<p className="text-[11px] text-muted-foreground mt-0.5">
						管理本地工作区，快速在智能体中打开
					</p>
				</div>
				<Button
					size="sm"
					className="h-7 text-xs gap-1.5"
					onClick={() => setAdding(true)}
				>
					<Plus className="size-3.5" />
					添加工作区
				</Button>
			</div>

			{/* Add form */}
			{adding && <AddRepoForm onDone={() => setAdding(false)} />}

			{/* Search */}
			{repos.length > 0 && (
				<div className="relative shrink-0">
					<Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
					<Input
						className="pl-8 h-8 text-[12px]"
						placeholder="搜索工作区名称、路径或语言..."
						value={query}
						onChange={(e) => setQuery(e.target.value)}
					/>
				</div>
			)}

			{/* Stats bar */}
			{repos.length > 0 && (
				<div className="flex items-center gap-4 text-[11px] text-muted-foreground shrink-0">
					<span>{repos.length} 个工作区</span>
					{repos.filter((r) => r.pinned).length > 0 && (
						<span className="flex items-center gap-1">
							<Pin className="size-3" />
							{repos.filter((r) => r.pinned).length} 个置顶
						</span>
					)}
					{query && (
						<span className="text-foreground/60">
							找到 {filtered.length} 个结果
						</span>
					)}
				</div>
			)}

			{/* List */}
			{filtered.length === 0 ? (
				<div className="flex flex-col items-center justify-center flex-1 text-muted-foreground gap-3">
					<GitBranch className="size-10 opacity-20" />
					<p className="text-sm">
						{query ? "没有匹配的工作区" : "还没有添加工作区"}
					</p>
					{!query && !adding && (
						<Button
							size="sm"
							variant="outline"
							className="h-7 text-xs gap-1.5"
							onClick={() => setAdding(true)}
						>
							<Plus className="size-3.5" />
							添加第一个工作区
						</Button>
					)}
				</div>
			) : (
				<div className="space-y-2">
					{filtered.map((repo) => (
						<RepoCard
							key={repo.id}
							repo={repo as Repo}
							onOpenInAgent={handleOpenInAgent}
							onOpenEditor={handleOpenEditor}
						/>
					))}
				</div>
			)}
		</div>
	);
}
