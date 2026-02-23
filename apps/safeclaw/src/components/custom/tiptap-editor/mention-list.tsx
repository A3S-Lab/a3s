/**
 * Shared suggestion popup for both @mention and /slash-command.
 * Rendered via TipTap's suggestion utility with tippy.js.
 */
import { cn } from "@/lib/utils";
import {
	forwardRef,
	useEffect,
	useImperativeHandle,
	useState,
	useCallback,
} from "react";

export interface SuggestionItem {
	id: string;
	label: string;
	description?: string;
	icon?: React.ReactNode;
	group?: string;
}

export interface MentionListRef {
	onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

interface MentionListProps {
	items: SuggestionItem[];
	command: (item: SuggestionItem) => void;
}

const MentionList = forwardRef<MentionListRef, MentionListProps>(
	({ items, command }, ref) => {
		const [selectedIndex, setSelectedIndex] = useState(0);

		useEffect(() => {
			setSelectedIndex(0);
		}, [items]);

		const selectItem = useCallback(
			(index: number) => {
				const item = items[index];
				if (item) command(item);
			},
			[items, command],
		);

		useImperativeHandle(ref, () => ({
			onKeyDown: ({ event }: { event: KeyboardEvent }) => {
				if (event.key === "ArrowUp") {
					event.preventDefault();
					setSelectedIndex((prev) => (prev - 1 + items.length) % items.length);
					return true;
				}
				if (event.key === "ArrowDown") {
					event.preventDefault();
					setSelectedIndex((prev) => (prev + 1) % items.length);
					return true;
				}
				if (event.key === "Enter" || event.key === "Tab") {
					event.preventDefault();
					selectItem(selectedIndex);
					return true;
				}
				if (event.key === "Escape") {
					return true;
				}
				return false;
			},
		}));

		if (items.length === 0) {
			return (
				<div className="rounded-lg border bg-popover text-popover-foreground shadow-lg p-2 w-64">
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
		items.forEach((item, idx) => {
			const g = item.group || "";
			if (g !== currentGroup) {
				currentGroup = g;
				groups.push({ key: g, items: [] });
			}
			groups[groups.length - 1].items.push({ ...item, globalIdx: idx });
		});

		return (
			<div className="rounded-lg border bg-popover text-popover-foreground shadow-lg py-1 w-72 max-h-64 overflow-y-auto">
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
								onClick={() => selectItem(item.globalIdx)}
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
								</div>
							</button>
						))}
					</div>
				))}
			</div>
		);
	},
);

MentionList.displayName = "MentionList";

export default MentionList;
