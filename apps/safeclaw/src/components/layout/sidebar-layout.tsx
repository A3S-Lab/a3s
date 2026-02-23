/**
 * Shared page layout components — reusable sidebar + section switching pattern.
 * Used by Settings, Security, and any future sidebar-based pages.
 */
import { cn } from "@/lib/utils";
import { ChevronRight, ShieldCheck } from "lucide-react";
import type { LucideIcon } from "lucide-react";

// =============================================================================
// Types
// =============================================================================

export interface SidebarSection<T extends string = string> {
	id: T;
	label: string;
	icon: LucideIcon;
	description: string;
}

export interface SidebarLayoutProps<T extends string> {
	/** Page title shown in sidebar header */
	title: string;
	/** Subtitle shown below title */
	subtitle: string;
	/** Navigation sections */
	sections: SidebarSection<T>[];
	/** Currently active section */
	current: T;
	/** Section change callback */
	onChange: (id: T) => void;
	/** Optional badge renderer per section (e.g. alert count) */
	badge?: (id: T) => React.ReactNode;
	/** Footer text (default: version string) */
	footer?: string;
	/** Main content max-width class (default: "max-w-3xl") */
	contentMaxWidth?: string;
	/** Section content renderer */
	children: React.ReactNode;
}

// =============================================================================
// SidebarLayout — generic sidebar + content page
// =============================================================================

export function SidebarLayout<T extends string>({
	title,
	subtitle,
	sections,
	current,
	onChange,
	badge,
	footer = "SafeClaw · a3s-code v0.9.0",
	contentMaxWidth = "max-w-3xl",
	children,
}: SidebarLayoutProps<T>) {
	return (
		<div className="flex h-full w-full">
			<nav
				aria-label={`${title} sections`}
				className="w-48 shrink-0 border-r border-border flex flex-col"
			>
				<div className="px-4 pt-4 pb-3">
					<h1 className="text-sm font-bold">{title}</h1>
					<p className="text-[11px] text-muted-foreground mt-0.5">{subtitle}</p>
				</div>
				<div className="px-2 space-y-0.5 flex-1">
					{sections.map((s) => {
						const active = current === s.id;
						return (
							<button
								key={s.id}
								onClick={() => onChange(s.id)}
								aria-current={active ? "page" : undefined}
								className={cn(
									"w-full flex items-center gap-2.5 text-left px-2.5 py-2 rounded-lg text-sm transition-all group",
									active
										? "bg-primary/10 text-primary"
										: "text-muted-foreground hover:text-foreground hover:bg-muted/50",
								)}
							>
								<div
									className={cn(
										"flex items-center justify-center size-7 rounded-md shrink-0",
										active ? "bg-primary/15" : "bg-muted group-hover:bg-muted",
									)}
								>
									<s.icon
										className={cn(
											"size-3.5",
											active
												? "text-primary"
												: "text-muted-foreground group-hover:text-foreground",
										)}
									/>
								</div>
								<div className="flex-1 min-w-0">
									<div className="font-medium text-[12px] leading-tight">
										{s.label}
									</div>
								</div>
								{badge?.(s.id)}
								<ChevronRight
									className={cn(
										"size-3 shrink-0 transition-opacity",
										active ? "opacity-60" : "opacity-0 group-hover:opacity-40",
									)}
								/>
							</button>
						);
					})}
				</div>
				<div className="px-4 py-3 border-t">
					<div className="flex items-center gap-2 text-[10px] text-muted-foreground/60">
						<ShieldCheck className="size-3" />
						<span>{footer}</span>
					</div>
				</div>
			</nav>
			<main className="flex-1 overflow-y-auto">
				<div className={cn(contentMaxWidth, "px-6 py-5")}>{children}</div>
			</main>
		</div>
	);
}

// =============================================================================
// SectionHeader — shared section title with icon
// =============================================================================

export function SectionHeader({
	title,
	description,
	icon: Icon,
}: {
	title: string;
	description: string;
	icon: LucideIcon;
}) {
	return (
		<div className="flex items-start gap-3 mb-6">
			<div className="flex items-center justify-center size-10 rounded-xl bg-primary/10 shrink-0 mt-0.5">
				<Icon className="size-5 text-primary" />
			</div>
			<div>
				<h2 className="text-lg font-bold">{title}</h2>
				<p className="text-sm text-muted-foreground mt-0.5">{description}</p>
			</div>
		</div>
	);
}

// =============================================================================
// StatCard — metric display card (used in security overview, diagnostics, etc.)
// =============================================================================

export function StatCard({
	icon: Icon,
	label,
	value,
	sub,
	color,
}: {
	icon: LucideIcon;
	label: string;
	value: string | number;
	sub?: string;
	color?: string;
}) {
	return (
		<div className="rounded-xl border bg-card p-5">
			<div className="flex items-center gap-2 mb-2">
				<Icon className={cn("size-4", color || "text-primary")} />
				<span className="text-xs text-muted-foreground">{label}</span>
			</div>
			<p className="text-2xl font-bold tabular-nums">{value}</p>
			{sub && <p className="text-[11px] text-muted-foreground mt-0.5">{sub}</p>}
		</div>
	);
}

// =============================================================================
// SettingRow — label + hint + input row (used in settings sections)
// =============================================================================

export function SettingRow({
	label,
	hint,
	children,
	action,
}: {
	label: string;
	hint?: string;
	children: React.ReactNode;
	action?: React.ReactNode;
}) {
	return (
		<div className="flex items-start justify-between gap-8 py-4 border-b border-border/50 last:border-b-0">
			<div className="shrink-0 min-w-[120px]">
				<div className="text-sm font-medium">{label}</div>
				{hint && (
					<p className="text-xs text-muted-foreground mt-0.5 leading-relaxed">
						{hint}
					</p>
				)}
			</div>
			<div className="flex-1 max-w-sm flex items-center gap-2">
				<div className="flex-1">{children}</div>
				{action}
			</div>
		</div>
	);
}

// =============================================================================
// Provider color helpers
// =============================================================================

export const PROVIDER_COLORS: Record<string, string> = {
	anthropic:
		"bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20",
	openai: "bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
	google: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20",
	deepseek:
		"bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 border-indigo-500/20",
};

export function pColor(n: string) {
	return (
		PROVIDER_COLORS[n] ||
		"bg-purple-500/10 text-purple-600 dark:text-purple-400 border-purple-500/20"
	);
}
