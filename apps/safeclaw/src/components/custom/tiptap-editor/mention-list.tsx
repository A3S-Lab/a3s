/**
 * Shared suggestion popup for both @mention and /slash-command.
 * Rendered via TipTap's suggestion utility with tippy.js.
 */
import { cn } from "@/lib/utils";
import { Search } from "lucide-react";
import {
	forwardRef,
	useCallback,
	useEffect,
	useImperativeHandle,
	useRef,
	useState,
} from "react";

export interface SuggestionItem {
	id: string;
	label: string;
	description?: string;
	icon?: React.ReactNode;
	group?: string;
	isDirectory?: boolean;
	expanded?: boolean;
	path?: string;
	level?: number;
}

export interface MentionListRef {
	onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

interface MentionListProps {
	items: SuggestionItem[];
	command: (item: SuggestionItem) => void;
	onFolderClick?: (item: SuggestionItem) => void;
	/** Enable search input (for @mention panel) */
	enableSearch?: boolean;
}

const MentionList = forwardRef<MentionListRef, MentionListProps>(
	({ items, command, onFolderClick, enableSearch }, ref) => {
		const [selectedIndex, setSelectedIndex] = useState(0);
		const [searchQuery, setSearchQuery] = useState("");
		const searchInputRef = useRef<HTMLInputElement>(null);
		const clickTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
		const clickCountRef = useRef<{ index: number; count: number }>({
			index: -1,
			count: 0,
		});

		// Filter items by search query
		const filteredItems = searchQuery
			? items.filter(
					(item) =>
						item.label.toLowerCase().includes(searchQuery.toLowerCase()) ||
						item.path?.toLowerCase().includes(searchQuery.toLowerCase()),
				)
			: items;

		// Clamp selectedIndex when items array changes length
		useEffect(() => {
			setSelectedIndex((prev) => {
				if (filteredItems.length === 0) return 0;
				return Math.min(prev, filteredItems.length - 1);
			});
		}, [filteredItems]);

		// Focus search input when component mounts and after re-renders
		useEffect(() => {
			if (enableSearch && searchInputRef.current) {
				// Use requestAnimationFrame to ensure DOM is ready
				requestAnimationFrame(() => {
					searchInputRef.current?.focus();
				});
			}
		});

		const selectItem = useCallback(
			(index: number) => {
				const item = filteredItems[index];
				if (item) command(item);
			},
			[filteredItems, command],
		);

		// Single click = expand/collapse folder, double click = select folder
		const handleItemClick = useCallback(
			(index: number) => {
				const item = filteredItems[index];
				if (!item) return;

				if (!item.isDirectory) {
					command(item);
					return;
				}

				if (clickCountRef.current.index === index) {
					clickCountRef.current.count++;
					if (clickCountRef.current.count === 2) {
						// Double click — select folder
						if (clickTimerRef.current) clearTimeout(clickTimerRef.current);
						clickCountRef.current = { index: -1, count: 0 };
						command(item);
					}
				} else {
					clickCountRef.current = { index, count: 1 };
					if (clickTimerRef.current) clearTimeout(clickTimerRef.current);
					clickTimerRef.current = setTimeout(() => {
						if (onFolderClick) onFolderClick(item);
						clickCountRef.current = { index: -1, count: 0 };
						clickTimerRef.current = null;
					}, 250);
				}
			},
			[filteredItems, command, onFolderClick],
		);

		useImperativeHandle(ref, () => ({
			onKeyDown: ({ event }: { event: KeyboardEvent }) => {
				if (event.key === "ArrowUp") {
					event.preventDefault();
					setSelectedIndex((prev) =>
						prev <= 0 ? filteredItems.length - 1 : prev - 1,
					);
					return true;
				}
				if (event.key === "ArrowDown") {
					event.preventDefault();
					setSelectedIndex((prev) =>
						prev >= filteredItems.length - 1 ? 0 : prev + 1,
					);
					return true;
				}
				if (event.key === "Enter" || event.key === "Tab") {
					event.preventDefault();
					const item = filteredItems[selectedIndex];
					if (item?.isDirectory) {
						if (onFolderClick) onFolderClick(item);
					} else {
						selectItem(selectedIndex);
					}
					return true;
				}
				if (event.key === "Escape") {
					return true;
				}
				return false;
			},
		}));

		if (filteredItems.length === 0) {
			return (
				<div className="rounded-lg border bg-popover text-popover-foreground shadow-lg p-2 w-72">
					{enableSearch && (
						<div className="relative mb-2">
							<Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-muted-foreground pointer-events-none" />
							<input
								ref={searchInputRef}
								type="text"
								value={searchQuery}
								onChange={(e) => setSearchQuery(e.target.value)}
								placeholder="搜索文件..."
								className="w-full h-7 pl-7 pr-2 text-xs border rounded bg-background outline-none focus:ring-1 focus:ring-ring"
							/>
						</div>
					)}
					<p className="text-xs text-muted-foreground px-2 py-1.5">
						无匹配结果
					</p>
				</div>
			);
		}

		// Group items
		const groups: {
			key: string;
			items: (SuggestionItem & { globalIdx: number })[];
		}[] = [];
		let currentGroup = "";
		filteredItems.forEach((item, idx) => {
			const g = item.group || "";
			if (g !== currentGroup) {
				currentGroup = g;
				groups.push({ key: g, items: [] });
			}
			groups[groups.length - 1].items.push({ ...item, globalIdx: idx });
		});

		return (
			<div className="rounded-lg border bg-popover text-popover-foreground shadow-lg py-1 w-72 max-h-80 flex flex-col">
				{enableSearch && (
					<div className="px-2 pt-2 pb-1 shrink-0">
						<div className="relative">
							<Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-muted-foreground pointer-events-none" />
							<input
								ref={searchInputRef}
								type="text"
								value={searchQuery}
								onChange={(e) => setSearchQuery(e.target.value)}
								placeholder="搜索文件..."
								className="w-full h-7 pl-7 pr-2 text-xs border rounded bg-background outline-none focus:ring-1 focus:ring-ring"
								onKeyDown={(e) => {
									// Prevent arrow keys from moving cursor in input
									if (
										e.key === "ArrowUp" ||
										e.key === "ArrowDown" ||
										e.key === "Enter" ||
										e.key === "Tab"
									) {
										e.preventDefault();
										// Trigger the ref's onKeyDown handler
										ref &&
											"current" in ref &&
											ref.current?.onKeyDown({ event: e.nativeEvent });
									}
								}}
							/>
						</div>
					</div>
				)}
				<div className="overflow-y-auto flex-1">
					{groups.map((group) => (
						<div key={group.key || "__default"}>
							{group.key && (
								<div className="px-3 pt-2 pb-1 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
									{group.key}
								</div>
							)}
							{group.items.map((item) => (
								<button
									key={item.id}
									type="button"
									className={cn(
										"flex items-center gap-2.5 w-full text-left px-3 py-1.5 text-xs transition-colors",
										item.globalIdx === selectedIndex
											? "bg-primary/10 text-primary"
											: "text-foreground hover:bg-foreground/[0.04]",
									)}
									style={
										item.level
											? { paddingLeft: `${12 + item.level * 12}px` }
											: undefined
									}
									onClick={() => handleItemClick(item.globalIdx)}
									onMouseEnter={() => setSelectedIndex(item.globalIdx)}
								>
									{item.icon && (
										<span className="size-5 flex items-center justify-center shrink-0 rounded bg-muted">
											{item.icon}
										</span>
									)}
									<div className="flex-1 min-w-0">
										<div className="font-medium truncate">{item.label}</div>
										{item.description && (
											<div className="text-[10px] text-muted-foreground truncate mt-0.5">
												{item.description}
											</div>
										)}
										{item.isDirectory && (
											<div className="text-[10px] text-muted-foreground truncate mt-0.5">
												{item.expanded ? "点击折叠" : "点击展开"} · 双击选择
											</div>
										)}
									</div>
								</button>
							))}
						</div>
					))}
				</div>
			</div>
		);
	},
);

MentionList.displayName = "MentionList";

export default MentionList;
