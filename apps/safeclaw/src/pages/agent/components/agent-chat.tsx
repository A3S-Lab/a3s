/**
 * Agent Chat — main chat component.
 * Split into focused sub-components under ./chat/.
 */
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import settingsModel, {
	resolveApiKey,
	resolveBaseUrl,
} from "@/models/settings.model";
import {
	connectSession,
	disconnectSession,
	sendToSession,
} from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import { ArrowDown, Circle, Loader2, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { useSnapshot } from "valtio";
import dayjs from "dayjs";
import type { AgentChatMessage } from "@/typings/agent";

// Persist first visible item index per session across remounts
const scrollIndexCache = new Map<string, number>();

import { chatMessageToRich } from "./chat/types";
import type { RichMessage } from "./chat/types";
import MessageItem, { DateSeparator } from "./chat/message-item";
import { StreamingDisplay } from "./chat/streaming-display";
import { ChatHeader } from "./chat/chat-header";
import { AgentInput } from "./chat/agent-input";
import {
	AuthStatusBanner,
	PermissionRequestPanel,
	AgentMessageInbox,
	EmptyChat,
} from "./chat/chat-panels";

export default function AgentChat({ sessionId }: { sessionId: string }) {
	const { messages, connectionStatus, sessionStatus, sdkSessions, streaming } =
		useSnapshot(agentModel.state);
	const rawMessages = messages[sessionId] || [];
	const isRunning = sessionStatus[sessionId] === "running";
	const isExited =
		sdkSessions.find((s) => s.session_id === sessionId)?.state === "exited";
	const [relaunching, setRelaunching] = useState(false);
	const [searchQuery, setSearchQuery] = useState("");
	const [showScrollBtn, setShowScrollBtn] = useState(false);

	const richMessages = useMemo(
		() => rawMessages.map((m) => chatMessageToRich(m as AgentChatMessage)),
		[rawMessages],
	);

	// Client-side message search filtering
	const displayMessages = useMemo(() => {
		if (!searchQuery.trim()) return richMessages;
		const q = searchQuery.toLowerCase();
		return richMessages.filter((msg) => {
			for (const block of msg.blocks) {
				if (block.type === "text" && block.content.toLowerCase().includes(q))
					return true;
				if (
					block.type === "tool_call" &&
					(block.tool.toLowerCase().includes(q) ||
						block.input.toLowerCase().includes(q) ||
						block.output?.toLowerCase().includes(q))
				)
					return true;
				if (
					block.type === "thinking" &&
					block.content.toLowerCase().includes(q)
				)
					return true;
			}
			return false;
		});
	}, [richMessages, searchQuery]);

	const virtuosoRef = useRef<VirtuosoHandle>(null);
	// true = user intentionally scrolled up, stop auto-follow
	const userScrolledUpRef = useRef(false);
	// Track first visible index for scroll persistence
	const firstVisibleIndexRef = useRef(0);

	// Resolve cached scroll index for this session (consumed once on mount)
	const cachedIndex = useRef(scrollIndexCache.get(sessionId));
	const hasRestoredPosition = cachedIndex.current != null;
	if (hasRestoredPosition) {
		userScrolledUpRef.current = true;
		scrollIndexCache.delete(sessionId);
	}

	// Save first visible index on unmount
	useEffect(() => {
		return () => {
			scrollIndexCache.set(sessionId, firstVisibleIndexRef.current);
		};
	}, [sessionId]);

	// Connect WebSocket on mount + clear unread
	useEffect(() => {
		connectSession(sessionId);
		agentModel.clearUnread(sessionId);
	}, [sessionId]);

	// Auto-scroll when new messages arrive
	useEffect(() => {
		agentModel.clearUnread(sessionId);
		if (!userScrolledUpRef.current) {
			virtuosoRef.current?.scrollToIndex({
				index: "LAST",
				align: "end",
				behavior: "smooth",
			});
		}
	}, [richMessages.length, sessionId]);

	// Auto-scroll during streaming — continuous follow
	const streamingText = streaming[sessionId];
	useEffect(() => {
		if (streamingText != null && !userScrolledUpRef.current) {
			virtuosoRef.current?.scrollToIndex({
				index: "LAST",
				align: "end",
				behavior: "auto",
			});
		}
	}, [streamingText]);

	// Scroll to bottom when entering running state (optimistic loading)
	const prevStatusRef = useRef<string | null>(null);
	useEffect(() => {
		const current = sessionStatus[sessionId] ?? null;
		if (current === "running" && prevStatusRef.current !== "running") {
			userScrolledUpRef.current = false;
			// Multiple attempts to ensure Footer is rendered and visible
			const t1 = setTimeout(() => {
				virtuosoRef.current?.scrollToIndex({
					index: "LAST",
					align: "end",
					behavior: "auto",
				});
			}, 50);
			const t2 = setTimeout(() => {
				virtuosoRef.current?.scrollToIndex({
					index: "LAST",
					align: "end",
					behavior: "auto",
				});
			}, 200);
			prevStatusRef.current = current;
			return () => {
				clearTimeout(t1);
				clearTimeout(t2);
			};
		}
		prevStatusRef.current = current;
	}, [sessionStatus[sessionId]]);

	// Send user message
	const handleSend = useCallback(
		async (text: string, images?: { media_type: string; data: string }[]) => {
			// Show loading immediately — don't wait for configure round-trip
			agentModel.setSessionStatus(sessionId, "running");
			// Initialize streaming key so Valtio tracks it for reactivity in StreamingDisplay
			agentModel.setStreaming(sessionId, "");
			agentModel.clearCompletedTools(sessionId);

			const modelId = settingsModel.state.defaultModel;
			const providerName = settingsModel.state.defaultProvider;
			const apiKey = resolveApiKey(providerName, modelId);
			const baseUrl = resolveBaseUrl(providerName, modelId);
			if (apiKey || baseUrl) {
				const fullModel =
					providerName && modelId ? `${providerName}/${modelId}` : modelId;
				try {
					await agentApi.configureSession(sessionId, {
						model: fullModel || undefined,
						api_key: apiKey || undefined,
						base_url: baseUrl || undefined,
					});
				} catch (e) {
					console.warn("Failed to configure session before send", e);
				}
			}
			const sent = sendToSession(sessionId, {
				type: "user_message",
				content: text,
				images,
			});
			if (!sent) {
				// WS not connected — revert optimistic state
				agentModel.setSessionStatus(sessionId, "idle");
				agentModel.setStreaming(sessionId, null);
			}
		},
		[sessionId],
	);

	// Retry: resend the last user message
	const handleRetry = useCallback(() => {
		if (isRunning) return;
		const lastUserMsg = [...rawMessages]
			.reverse()
			.find((m) => (m as AgentChatMessage).role === "user") as
			| AgentChatMessage
			| undefined;
		if (lastUserMsg) {
			handleSend(lastUserMsg.content, lastUserMsg.images);
		}
	}, [rawMessages, isRunning, handleSend]);

	const handleRelaunchFromBanner = useCallback(async () => {
		setRelaunching(true);
		try {
			const result = await agentApi.relaunchSession(sessionId);
			if (result?.session_id) {
				const sessions = await agentApi.listSessions();
				if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
				const existingPersonaId = personaModel.state.sessionPersonas[sessionId];
				if (existingPersonaId)
					personaModel.setSessionPersona(result.session_id, existingPersonaId);
				disconnectSession(sessionId);
				connectSession(result.session_id);
				agentModel.setCurrentSession(result.session_id);
			}
		} finally {
			setRelaunching(false);
		}
	}, [sessionId]);

	// Render item with date separator
	const renderItem = useCallback(
		(index: number, msg: RichMessage) => {
			const prev = index > 0 ? displayMessages[index - 1] : null;
			const msgDate = dayjs(msg.timestamp);
			const isValidDate = msgDate.isAfter("2000-01-01");
			const showDate =
				isValidDate && (!prev || !msgDate.isSame(dayjs(prev.timestamp), "day"));
			return (
				<>
					{showDate && <DateSeparator timestamp={msg.timestamp} />}
					<MessageItem msg={msg} sessionId={sessionId} onRetry={handleRetry} />
				</>
			);
		},
		[sessionId, displayMessages, handleRetry],
	);

	// Virtuoso atBottom tracking — detect user scroll intent
	const handleAtBottom = useCallback((atBottom: boolean) => {
		if (atBottom) {
			userScrolledUpRef.current = false;
		}
		setShowScrollBtn(!atBottom);
	}, []);

	// Track visible range for scroll position persistence
	const handleRangeChanged = useCallback(
		(range: { startIndex: number; endIndex: number }) => {
			firstVisibleIndexRef.current = range.startIndex;
		},
		[],
	);

	// Virtuoso scroller ref — detect upward scroll
	const handleScrollerRef = useCallback(
		(scroller: HTMLElement | Window | null) => {
			if (!scroller || scroller === window) return;
			const el = scroller as HTMLElement;
			if ((el as any).__scrollBound) return;
			(el as any).__scrollBound = true;
			let lastScrollTop = el.scrollTop;
			el.addEventListener(
				"scroll",
				() => {
					if (el.scrollTop < lastScrollTop - 5) {
						userScrolledUpRef.current = true;
					}
					lastScrollTop = el.scrollTop;
				},
				{ passive: true },
			);
		},
		[],
	);

	const forceScrollToBottom = useCallback(() => {
		userScrolledUpRef.current = false;
		virtuosoRef.current?.scrollToIndex({
			index: "LAST",
			align: "end",
			behavior: "smooth",
		});
	}, []);

	// Footer component — StreamingDisplay inside Virtuoso's scroll container
	const virtuosoComponents = useMemo(
		() => ({ Footer: () => <StreamingDisplay sessionId={sessionId} /> }),
		[sessionId],
	);

	// Keyboard shortcut: Cmd/Ctrl+End to scroll to bottom
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === "End") {
				e.preventDefault();
				forceScrollToBottom();
			}
		};
		window.addEventListener("keydown", handler);
		return () => window.removeEventListener("keydown", handler);
	}, [forceScrollToBottom]);

	return (
		<ResizablePanelGroup direction="vertical" className="h-full">
			<ResizablePanel className="flex flex-col overflow-hidden">
				<ChatHeader
					sessionId={sessionId}
					searchQuery={searchQuery}
					onSearchChange={setSearchQuery}
				/>
				{searchQuery && (
					<div className="px-3 py-1 bg-muted/30 border-b text-[11px] text-muted-foreground shrink-0">
						找到 {displayMessages.length} / {richMessages.length} 条消息
					</div>
				)}
				{isExited && (
					<div className="flex items-center gap-2 px-3 py-2 bg-muted/50 border-b text-xs text-muted-foreground shrink-0">
						<Circle className="size-2 fill-muted-foreground/40 text-muted-foreground/40 shrink-0" />
						<span>会话已退出</span>
						<button
							type="button"
							className="ml-auto flex items-center gap-1 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
							onClick={handleRelaunchFromBanner}
							disabled={relaunching}
						>
							{relaunching ? (
								<Loader2 className="size-3 animate-spin" />
							) : (
								<RefreshCw className="size-3" />
							)}
							重启
						</button>
					</div>
				)}
				<div
					className="relative flex-1 min-h-0"
					role="log"
					aria-live="polite"
					aria-label="Chat messages"
				>
					{displayMessages.length === 0 && !searchQuery && !isRunning ? (
						<EmptyChat sessionId={sessionId} />
					) : displayMessages.length === 0 && searchQuery ? (
						<div className="flex items-center justify-center h-full text-sm text-muted-foreground">
							没有匹配的消息
						</div>
					) : (
						<Virtuoso
							ref={virtuosoRef}
							scrollerRef={handleScrollerRef}
							className="h-full"
							data={displayMessages}
							itemContent={renderItem}
							{...(hasRestoredPosition
								? { initialTopMostItemIndex: cachedIndex.current! }
								: {})}
							rangeChanged={handleRangeChanged}
							followOutput={(isAtBottom) => {
								if (!userScrolledUpRef.current) return "smooth";
								return isAtBottom ? "smooth" : false;
							}}
							atBottomStateChange={handleAtBottom}
							atBottomThreshold={30}
							components={virtuosoComponents}
						/>
					)}

					{/* Scroll-to-bottom FAB */}
					{showScrollBtn && displayMessages.length > 0 && (
						<button
							type="button"
							className="absolute bottom-3 right-3 flex items-center justify-center size-8 rounded-full border bg-background/95 backdrop-blur shadow-md text-muted-foreground hover:text-foreground hover:bg-background transition-all z-20"
							onClick={forceScrollToBottom}
							aria-label="滚动到底部"
						>
							<ArrowDown className="size-4" />
						</button>
					)}
				</div>
				<AgentMessageInbox sessionId={sessionId} />
				<AuthStatusBanner sessionId={sessionId} />
				<PermissionRequestPanel sessionId={sessionId} />
			</ResizablePanel>
			<ResizableHandle />
			<ResizablePanel defaultSize={20} minSize={10} maxSize={35}>
				<AgentInput
					sessionId={sessionId}
					disabled={isRunning}
					onSend={handleSend}
				/>
			</ResizablePanel>
		</ResizablePanelGroup>
	);
}
