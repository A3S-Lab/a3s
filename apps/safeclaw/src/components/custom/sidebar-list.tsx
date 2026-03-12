/**
 * Generic sidebar list component
 * Used for agent list, knowledge base list, etc.
 */
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { Search } from "lucide-react";
import { ReactNode } from "react";

interface SidebarListProps {
	/** List title */
	title: string;
	/** List width in pixels */
	width?: number;
	/** List items */
	children: ReactNode;
	/** Optional header actions (e.g., add button) */
	headerActions?: ReactNode;
	/** Optional search component */
	searchComponent?: ReactNode;
	/** Enable search functionality */
	enableSearch?: boolean;
	/** Search placeholder text */
	searchPlaceholder?: string;
	/** Search value */
	searchValue?: string;
	/** Search change handler */
	onSearchChange?: (value: string) => void;
	/** Additional container class names */
	className?: string;
}

export function SidebarList({
	title,
	width,
	children,
	headerActions,
	searchComponent,
	enableSearch = false,
	searchPlaceholder = "搜索...",
	searchValue = "",
	onSearchChange,
	className,
}: SidebarListProps) {
	return (
		<div
			className={cn(
				"flex flex-col h-full overflow-hidden border-r shrink-0",
				className,
			)}
			style={width ? { width: `${width}px` } : undefined}
		>
			{/* Header */}
			<div className="flex items-center justify-between px-3 py-3 border-b">
				<h2 className="text-sm font-semibold truncate">{title}</h2>
				{headerActions}
			</div>

			{/* Search */}
			{enableSearch && (
				<div className="px-3 py-2 border-b">
					<div className="relative">
						<Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
						<Input
							placeholder={searchPlaceholder}
							value={searchValue}
							onChange={(e) => onSearchChange?.(e.target.value)}
							className="pl-8 h-9"
						/>
					</div>
				</div>
			)}

			{/* Optional custom search component */}
			{searchComponent}

			{/* List content */}
			<ScrollArea className="flex-1 w-full">{children}</ScrollArea>
		</div>
	);
}

interface SidebarListItemProps {
	/** Whether this item is selected */
	selected?: boolean;
	/** Click handler */
	onClick?: () => void;
	/** Item content */
	children: ReactNode;
	/** Additional class names */
	className?: string;
}

export function SidebarListItem({
	selected,
	onClick,
	children,
	className,
}: SidebarListItemProps) {
	return (
		<div
			role="option"
			aria-selected={selected}
			className={cn(
				"px-3 py-3 cursor-pointer transition-colors hover:bg-accent/[0.08]",
				selected && "bg-primary/[0.08]",
				className,
			)}
			onClick={onClick}
		>
			{children}
		</div>
	);
}

interface SidebarListEmptyProps {
	/** Empty state message */
	message: string;
}

export function SidebarListEmpty({ message }: SidebarListEmptyProps) {
	return (
		<div className="px-3 py-8 text-center text-sm text-muted-foreground">
			{message}
		</div>
	);
}
