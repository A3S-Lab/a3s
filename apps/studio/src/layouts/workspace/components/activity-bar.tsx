import { cn } from "@/lib/utils";
import { LayoutDashboard, Settings } from "lucide-react";
import {
	ReactNode,
	useCallback,
	useRef,
	KeyboardEvent,
} from "react";
import { useLocation, useNavigate } from "react-router-dom";

const STORAGE_KEY = "studio-active-route";

const NAV_ITEMS = [
	{ key: "home", label: "工作台", icon: LayoutDashboard, path: "/" },
] as const;

const BOTTOM_ITEMS = [
	{ key: "settings", label: "设置", icon: Settings, path: "/settings" },
] as const;

const ALL_KEYS: string[] = [...NAV_ITEMS, ...BOTTOM_ITEMS].map((i) => i.key);

const ROUTE_MAP: Record<string, string> = Object.fromEntries(
	[...NAV_ITEMS, ...BOTTOM_ITEMS].map((i) => [i.key, i.path]),
);

function pathToKey(pathname: string): string {
	const segment = pathname.replace(/^\//, "").split("/")[0] || "home";
	return segment in ROUTE_MAP ? segment : "home";
}

interface ActivityItemProps {
	isActive: boolean;
	icon: ReactNode;
	label: string;
	onClick: () => void;
	onKeyDown: (e: KeyboardEvent) => void;
	tabIndex: number;
	itemRef: (el: HTMLButtonElement | null) => void;
}

const ActivityItem = ({
	icon,
	isActive,
	label,
	onClick,
	onKeyDown,
	tabIndex,
	itemRef,
}: ActivityItemProps) => {
	return (
		<button
			ref={itemRef}
			type="button"
			role="tab"
			aria-selected={isActive}
			aria-label={label}
			tabIndex={tabIndex}
			className={cn(
				"relative flex flex-col justify-center items-center w-full h-12 cursor-pointer transition-all duration-200",
				"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset",
				isActive
					? "text-primary-foreground"
					: "text-primary-foreground/40 hover:text-primary-foreground/80",
			)}
			onClick={onClick}
			onKeyDown={onKeyDown}
		>
			{/* Active indicator — left edge accent */}
			<span
				className={cn(
					"absolute left-0 top-1/2 -translate-y-1/2 w-[3px] rounded-r-full bg-primary-foreground transition-all duration-200",
					isActive ? "h-5 opacity-100" : "h-0 opacity-0",
				)}
			/>
			{/* Icon container with glow background */}
			<div
				className={cn(
					"flex items-center justify-center size-8 rounded-lg transition-all duration-200",
					isActive
						? "bg-primary-foreground/15 shadow-[0_0_8px_rgba(255,255,255,0.1)]"
						: "hover:bg-primary-foreground/8",
				)}
			>
				<div className="size-[18px]">{icon}</div>
			</div>
		</button>
	);
};

export default function ActivityBar() {
	const location = useLocation();
	const nav = useNavigate();
	const activeKey = pathToKey(location.pathname);
	const itemRefs = useRef<Map<string, HTMLButtonElement>>(new Map());

	const handleNavigate = useCallback(
		(key: string) => {
			const path = ROUTE_MAP[key] ?? "/";
			nav(path);
			try {
				localStorage.setItem(STORAGE_KEY, key);
			} catch {
				// Storage unavailable
			}
		},
		[nav],
	);

	const handleKeyDown = useCallback((e: KeyboardEvent, currentKey: string) => {
		const idx = ALL_KEYS.indexOf(currentKey);
		let nextIdx = -1;

		if (e.key === "ArrowDown") {
			e.preventDefault();
			nextIdx = (idx + 1) % ALL_KEYS.length;
		} else if (e.key === "ArrowUp") {
			e.preventDefault();
			nextIdx = (idx - 1 + ALL_KEYS.length) % ALL_KEYS.length;
		} else if (e.key === "Home") {
			e.preventDefault();
			nextIdx = 0;
		} else if (e.key === "End") {
			e.preventDefault();
			nextIdx = ALL_KEYS.length - 1;
		}

		if (nextIdx >= 0) {
			const nextKey = ALL_KEYS[nextIdx];
			itemRefs.current.get(nextKey)?.focus();
		}
	}, []);

	const setRef = (key: string) => (el: HTMLButtonElement | null) => {
		if (el) itemRefs.current.set(key, el);
		else itemRefs.current.delete(key);
	};

	return (
		<nav
			aria-label="Main navigation"
			className="flex flex-col h-full w-[var(--activity-bar-width)] bg-primary text-primary-foreground/60 shadow-lg"
		>
			{/* Logo area */}
			<div className="flex items-center justify-center h-12">
				<span className="text-primary-foreground font-bold text-sm">S</span>
			</div>
			<div
				className="flex-1 flex flex-col"
				role="tablist"
				aria-orientation="vertical"
			>
				<div className="flex-1">
					{NAV_ITEMS.map((item) => (
						<ActivityItem
							key={item.key}
							icon={
								<item.icon
									className="size-[18px]"
									strokeWidth={activeKey === item.key ? 2.2 : 1.8}
								/>
							}
							isActive={activeKey === item.key}
							label={item.label}
							tabIndex={activeKey === item.key ? 0 : -1}
							onClick={() => handleNavigate(item.key)}
							onKeyDown={(e) => handleKeyDown(e, item.key)}
							itemRef={setRef(item.key)}
						/>
					))}
				</div>
				<div className="pb-2">
					{BOTTOM_ITEMS.map((item) => (
						<ActivityItem
							key={item.key}
							icon={
								<item.icon
									className="size-[18px]"
									strokeWidth={activeKey === item.key ? 2.2 : 1.8}
								/>
							}
							isActive={activeKey === item.key}
							label={item.label}
							tabIndex={activeKey === item.key ? 0 : -1}
							onClick={() => handleNavigate(item.key)}
							onKeyDown={(e) => handleKeyDown(e, item.key)}
							itemRef={setRef(item.key)}
						/>
					))}
				</div>
			</div>
		</nav>
	);
}
