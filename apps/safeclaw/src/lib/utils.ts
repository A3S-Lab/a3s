import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function isMarkdown(text: string) {
	const markdownPatterns = [
		/^#{1,6}\s/, // 标题
		/^-|\*|\+|\d+\.\s/, // 列表
		/\[(.*?)\]\((.*?)\)/, // 链接
		/\*\*(.*?)\*\*|__(.*?)__/, // 强调
		// biome-ignore lint/correctness/noEmptyCharacterClassInRegex: <explanation>
		/```[^]*```|`[^`]*`/, // 代码
		/^> /, // 引用
		/!\[(.*?)\]\((.*?)\)/, // 图片
		/\|[^|]+/g, // 表格
		/([-*_]){3,}/, // 水平线
	];

	return markdownPatterns.some((pattern) => pattern.test(text));
}
