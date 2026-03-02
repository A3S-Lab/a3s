import { useState, useEffect, useRef, useCallback } from "react";
import {
	Radio,
	RefreshCw,
	Bookmark,
	BookmarkCheck,
	ChevronDown,
	TrendingUp,
	Globe,
	DollarSign,
	Cpu,
	Newspaper,
	AlertCircle,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { getGatewayUrl } from "@/models/settings.model";

type Category = "全部" | "时政" | "财经" | "科技" | "国际";

interface NewsItem {
	id: string;
	category: Category;
	source: string;
	title: string;
	summary: string;
	time: string;
	hot: boolean;
	saved: boolean;
	tags: string[];
	url?: string;
}

const CATEGORY_ICONS: Record<Category, typeof Globe> = {
	全部: Newspaper,
	时政: Globe,
	财经: DollarSign,
	科技: Cpu,
	国际: TrendingUp,
};

const CATEGORY_COLORS: Record<Category, string> = {
	全部: "bg-muted text-muted-foreground",
	时政: "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
	财经: "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
	科技: "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
	国际: "bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400",
};

const FETCH_PROMPT = `今天是 ${new Date().toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric" })}。

请使用搜索工具搜索今日最新新闻资讯，覆盖以下分类：时政、财经、科技、国际。每个分类搜索2-3条最新、最重要的新闻。

搜索完成后，请严格按照以下 JSON 格式输出，不要输出任何其他内容：

\`\`\`json
[
  {
    "id": "唯一ID",
    "category": "时政|财经|科技|国际",
    "source": "来源媒体",
    "title": "新闻标题",
    "summary": "100字左右的摘要",
    "time": "X小时前 或 X分钟前",
    "hot": true或false,
    "tags": ["标签1", "标签2"]
  }
]
\`\`\`

要求：
- 必须是真实的今日新闻，通过搜索获取
- 每条新闻 summary 控制在80-120字
- hot 字段：重要程度高的标记为 true，每个分类最多1条
- tags 每条2-3个关键词
- 直接输出 JSON，不要有任何前缀说明`;

type FetchState = "idle" | "connecting" | "fetching" | "done" | "error";

function useNewsSession() {
	const [news, setNews] = useState<NewsItem[]>([]);
	const [state, setState] = useState<FetchState>("idle");
	const [error, setError] = useState<string | null>(null);
	const wsRef = useRef<WebSocket | null>(null);
	const sessionIdRef = useRef<string | null>(null);
	const bufferRef = useRef("");
	const stateRef = useRef<FetchState>("idle");

	const parseNewsFromText = useCallback((text: string): NewsItem[] | null => {
		const match = text.match(/```json\s*([\s\S]*?)```/);
		if (!match) return null;
		try {
			const parsed = JSON.parse(match[1]) as Omit<NewsItem, "saved">[];
			return parsed.map((item) => ({ ...item, saved: false }));
		} catch {
			return null;
		}
	}, []);

	const fetchNews = useCallback(async () => {
		if (stateRef.current === "connecting" || stateRef.current === "fetching")
			return;
		// Close existing ws
		if (wsRef.current) {
			wsRef.current.close();
			wsRef.current = null;
		}

		setState("connecting");
		stateRef.current = "connecting";
		setError(null);
		bufferRef.current = "";

		const gatewayUrl = getGatewayUrl();

		// Create a new agent session
		try {
			const res = await fetch(`${gatewayUrl}/api/agent/sessions`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ name: "消息雷达", cwd: "/" }),
			});
			if (!res.ok) throw new Error(`创建 session 失败: ${res.status}`);
			const data = (await res.json()) as { session_id: string };
			sessionIdRef.current = data.session_id;
		} catch (e) {
			setState("error");
			setError(e instanceof Error ? e.message : "无法连接到 AI 服务");
			return;
		}

		// Connect WebSocket
		const wsUrl = `${gatewayUrl.replace(/^http/, "ws")}/ws/agent/browser/${sessionIdRef.current}`;
		const ws = new WebSocket(wsUrl);
		wsRef.current = ws;

		ws.onopen = () => {
			setState("fetching");
		};

		ws.onmessage = (event) => {
			try {
				const msg = JSON.parse(event.data as string) as Record<string, unknown>;

				if (msg.type === "cli_connected") {
					// Send the search prompt
					ws.send(
						JSON.stringify({ type: "user_message", content: FETCH_PROMPT }),
					);
				}

				if (msg.type === "stream_event") {
					const ev = msg.event as Record<string, unknown>;
					if (ev.type === "content_block_delta") {
						const delta = ev.delta as Record<string, unknown>;
						if (delta?.type === "text_delta") {
							bufferRef.current += delta.text as string;
						}
					}
				}

				if (msg.type === "assistant") {
					const message = msg.message as {
						content: { type: string; text?: string }[];
						stop_reason: string | null;
					};
					if (message.stop_reason === "end_turn") {
						const fullText = message.content
							.filter((b) => b.type === "text")
							.map((b) => b.text ?? "")
							.join("");
						const parsed = parseNewsFromText(fullText || bufferRef.current);
						if (parsed && parsed.length > 0) {
							setNews(parsed);
							setState("done");
							stateRef.current = "done";
						} else {
							setState("error");
							stateRef.current = "error";
							setError("AI 返回的数据格式不正确，请重试");
						}
						ws.close();
					}
				}

				if (msg.type === "error") {
					setState("error");
					setError((msg.message as string) || "AI 服务出错");
					ws.close();
				}
			} catch {
				// ignore parse errors
			}
		};

		ws.onerror = () => {
			setState("error");
			setError("WebSocket 连接失败");
		};

		ws.onclose = () => {
			if (stateRef.current === "fetching") {
				const parsed = parseNewsFromText(bufferRef.current);
				if (parsed && parsed.length > 0) {
					setNews(parsed);
					setState("done");
					stateRef.current = "done";
				}
			}
		};
	}, [parseNewsFromText]);

	// Cleanup on unmount
	useEffect(() => {
		return () => {
			wsRef.current?.close();
		};
	}, []);

	return { news, setNews, state, error, fetchNews };
}

export default function MessageRadarPage() {
	const { news, setNews, state, error, fetchNews } = useNewsSession();
	const [category, setCategory] = useState<Category>("全部");
	const [expanded, setExpanded] = useState<string | null>(null);
	const [showSaved, setShowSaved] = useState(false);

	// Auto-fetch on mount
	useEffect(() => {
		fetchNews();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	const filtered = news.filter((n) => {
		if (showSaved) return n.saved;
		if (category !== "全部" && n.category !== category) return false;
		return true;
	});

	const toggleSave = (id: string) => {
		setNews((prev) =>
			prev.map((n) => (n.id === id ? { ...n, saved: !n.saved } : n)),
		);
	};

	const savedCount = news.filter((n) => n.saved).length;
	const isLoading = state === "connecting" || state === "fetching";

	return (
		<div className="flex flex-col h-screen bg-background text-foreground">
			{/* Header */}
			<div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-background/95 backdrop-blur shrink-0">
				<div className="size-8 rounded-xl bg-gradient-to-br from-blue-500 to-blue-600 flex items-center justify-center">
					<Radio className="size-4 text-white" />
				</div>
				<div>
					<h1 className="text-sm font-semibold leading-none">消息雷达</h1>
					<p className="text-[10px] text-muted-foreground mt-0.5">
						{isLoading
							? "AI 正在搜索最新资讯..."
							: `AI 资讯聚合 · ${new Date().toLocaleDateString("zh-CN")} 更新`}
					</p>
				</div>
				<div className="flex-1" />
				<button
					type="button"
					onClick={() => setShowSaved(!showSaved)}
					className={cn(
						"flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-lg transition-colors",
						showSaved
							? "bg-primary text-primary-foreground"
							: "hover:bg-muted text-muted-foreground",
					)}
				>
					<Bookmark className="size-3" />
					{savedCount > 0 && <span>{savedCount}</span>}
				</button>
				<button
					type="button"
					onClick={fetchNews}
					disabled={isLoading}
					className="p-1.5 rounded-lg hover:bg-muted transition-colors disabled:opacity-50"
					title="刷新"
				>
					<RefreshCw
						className={cn("size-4", isLoading && "animate-spin text-blue-500")}
					/>
				</button>
			</div>

			{/* Category tabs */}
			{!showSaved && (
				<div className="flex gap-1.5 px-4 py-2 border-b border-border overflow-x-auto shrink-0">
					{(["全部", "时政", "财经", "科技", "国际"] as Category[]).map(
						(cat) => {
							const Icon = CATEGORY_ICONS[cat];
							return (
								<button
									key={cat}
									type="button"
									onClick={() => setCategory(cat)}
									className={cn(
										"flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-full whitespace-nowrap transition-colors shrink-0",
										category === cat
											? "bg-primary text-primary-foreground"
											: "bg-muted text-muted-foreground hover:bg-muted/80",
									)}
								>
									<Icon className="size-3" />
									{cat}
								</button>
							);
						},
					)}
				</div>
			)}

			{/* Hot strip */}
			{!showSaved &&
				category === "全部" &&
				news.filter((n) => n.hot).length > 0 && (
					<div className="px-4 py-2 border-b border-border bg-red-50/50 dark:bg-red-950/10 shrink-0">
						<div className="flex items-center gap-2 overflow-x-auto">
							<span className="text-[10px] font-medium text-red-600 dark:text-red-400 shrink-0 flex items-center gap-1">
								<TrendingUp className="size-3" /> 热点
							</span>
							{news
								.filter((n) => n.hot)
								.map((n) => (
									<button
										key={n.id}
										type="button"
										onClick={() => setExpanded(n.id)}
										className="text-[10px] text-foreground/70 hover:text-foreground whitespace-nowrap transition-colors shrink-0"
									>
										{n.title.slice(0, 18)}…
									</button>
								))}
						</div>
					</div>
				)}

			{/* Content */}
			<div className="flex-1 overflow-y-auto">
				{isLoading ? (
					<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
						<div className="relative">
							<Radio className="size-10 opacity-20" />
							<div className="absolute inset-0 flex items-center justify-center">
								<div className="size-12 rounded-full border-2 border-blue-500/30 border-t-blue-500 animate-spin" />
							</div>
						</div>
						<div className="text-center">
							<p className="text-sm font-medium">AI 正在搜索今日资讯</p>
							<p className="text-xs text-muted-foreground/60 mt-1">
								正在获取时政、财经、科技、国际最新动态...
							</p>
						</div>
					</div>
				) : state === "error" ? (
					<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground px-6 text-center">
						<AlertCircle className="size-8 text-destructive/50" />
						<p className="text-sm">{error ?? "获取资讯失败"}</p>
						<button
							type="button"
							onClick={fetchNews}
							className="text-xs px-3 py-1.5 rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
						>
							重试
						</button>
					</div>
				) : filtered.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-2">
						<Newspaper className="size-8 opacity-30" />
						<p className="text-sm">{showSaved ? "暂无收藏" : "暂无资讯"}</p>
					</div>
				) : (
					<div className="divide-y divide-border">
						{filtered.map((item) => {
							const isOpen = expanded === item.id;
							return (
								<div
									key={item.id}
									className="px-4 py-3 hover:bg-muted/30 transition-colors"
								>
									<button
										type="button"
										className="w-full text-left"
										onClick={() => setExpanded(isOpen ? null : item.id)}
									>
										<div className="flex items-start gap-2 mb-1.5">
											<span
												className={cn(
													"text-[10px] px-1.5 py-0.5 rounded-full shrink-0 mt-0.5",
													CATEGORY_COLORS[item.category],
												)}
											>
												{item.category}
											</span>
											{item.hot && (
												<span className="text-[10px] px-1.5 py-0.5 rounded-full bg-red-500 text-white shrink-0 mt-0.5">
													热
												</span>
											)}
											<p className="text-xs font-medium leading-snug flex-1 text-left">
												{item.title}
											</p>
										</div>
										<p
											className={cn(
												"text-xs text-muted-foreground leading-relaxed",
												!isOpen && "line-clamp-2",
											)}
										>
											{item.summary}
										</p>
									</button>

									<div className="flex items-center gap-2 mt-2">
										<span className="text-[10px] text-muted-foreground">
											{item.source}
										</span>
										<span className="text-[10px] text-muted-foreground">·</span>
										<span className="text-[10px] text-muted-foreground">
											{item.time}
										</span>
										<div className="flex gap-1 flex-wrap flex-1">
											{item.tags.map((tag) => (
												<span
													key={tag}
													className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground"
												>
													{tag}
												</span>
											))}
										</div>
										<button
											type="button"
											onClick={(e) => {
												e.stopPropagation();
												toggleSave(item.id);
											}}
											className="p-1 rounded hover:bg-muted transition-colors shrink-0"
										>
											{item.saved ? (
												<BookmarkCheck className="size-3.5 text-primary" />
											) : (
												<Bookmark className="size-3.5 text-muted-foreground" />
											)}
										</button>
										<ChevronDown
											className={cn(
												"size-3 text-muted-foreground transition-transform shrink-0",
												isOpen && "rotate-180",
											)}
										/>
									</div>
								</div>
							);
						})}
					</div>
				)}
			</div>

			{/* Footer */}
			{state === "done" && (
				<div className="px-4 py-2 border-t border-border bg-muted/30 shrink-0">
					<p className="text-[10px] text-muted-foreground text-center">
						共 {news.length} 条资讯 · AI 实时搜索生成 ·{" "}
						{new Date().toLocaleDateString("zh-CN")} 更新
					</p>
				</div>
			)}
		</div>
	);
}
