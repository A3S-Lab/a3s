/**
 * Shared time formatting utilities.
 * Replaces 7 duplicated timeAgo/relativeTime implementations across pages.
 */

/** Relative time string (e.g. "3 分钟前", "2 天前") */
export function timeAgo(ts: number): string {
	const diff = Date.now() - ts;
	if (diff < 60_000) return "刚刚";
	if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
	if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
	if (diff < 86_400_000 * 30) return `${Math.floor(diff / 86_400_000)} 天前`;
	return new Date(ts).toLocaleDateString();
}

/** Format timestamp to locale string */
export function formatDate(ts: number): string {
	return new Date(ts).toLocaleString();
}
