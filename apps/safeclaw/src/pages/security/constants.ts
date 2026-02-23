/**
 * Shared constants for security pages — severity colors, labels, vector labels, TEE labels.
 */
import type { Severity } from "@/models/security.model";

export const SEVERITY_COLORS: Record<Severity, string> = {
	info: "bg-primary/10 text-primary border-primary/20",
	warning:
		"bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
	high: "bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20",
	critical: "bg-destructive/10 text-destructive border-destructive/20",
};

export const SEVERITY_DOT: Record<Severity, string> = {
	info: "bg-primary",
	warning: "bg-amber-500",
	high: "bg-orange-500",
	critical: "bg-destructive",
};

export const SEVERITY_LABELS: Record<Severity, string> = {
	info: "信息",
	warning: "警告",
	high: "高危",
	critical: "严重",
};

export const VECTOR_LABELS: Record<string, string> = {
	OutputChannel: "输出泄露",
	ToolCall: "工具调用",
	DangerousCommand: "危险命令",
	NetworkExfil: "网络外发",
	FileExfil: "文件外泄",
	AuthFailure: "认证失败",
};

export const TEE_COLORS: Record<string, string> = {
	TeeHardware: "text-green-600 dark:text-green-400",
	VmIsolation: "text-amber-600 dark:text-amber-400",
	ProcessOnly: "text-red-600 dark:text-red-400",
};

export const TEE_LABELS: Record<string, string> = {
	TeeHardware: "硬件隔离",
	VmIsolation: "虚拟机隔离",
	ProcessOnly: "进程隔离",
};
