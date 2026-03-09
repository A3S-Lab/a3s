import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { useReactive } from "ahooks";
import { Code2, Download, ExternalLink, Search, Star, Zap } from "lucide-react";

// Mock data - 实际应该从 API 获取
const CATEGORIES = [
	{ id: "all", label: "全部" },
	{ id: "regular", label: "常规技能" },
	{ id: "security", label: "安全技能" },
];

interface Skill {
	id: string;
	name: string;
	description: string;
	category: string;
	author: string;
	icon: string;
	downloads: number;
	rating: number;
	tags: string[];
	installed: boolean;
	version: string;
}

const MOCK_SKILLS: Skill[] = [
	{
		id: "1",
		name: "GitHub 集成",
		description: "与 GitHub 深度集成，支持仓库管理、Issue 追踪、PR 审查等功能",
		category: "regular",
		author: "A3S Lab",
		icon: "github",
		downloads: 23456,
		rating: 4.9,
		tags: ["GitHub", "版本控制", "代码托管"],
		installed: true,
		version: "1.2.0",
	},
	{
		id: "2",
		name: "数据库查询",
		description:
			"支持多种数据库的查询和管理，包括 MySQL、PostgreSQL、MongoDB 等",
		category: "regular",
		author: "DataTools",
		icon: "database",
		downloads: 18234,
		rating: 4.7,
		tags: ["数据库", "SQL", "查询"],
		installed: false,
		version: "2.0.1",
	},
	{
		id: "3",
		name: "Slack 通知",
		description: "发送消息到 Slack 频道，支持富文本、附件、交互式消息等",
		category: "regular",
		author: "SlackBot",
		icon: "slack",
		downloads: 15678,
		rating: 4.6,
		tags: ["Slack", "通知", "协作"],
		installed: false,
		version: "1.5.2",
	},
	{
		id: "4",
		name: "网页爬虫",
		description: "强大的网页数据抓取工具，支持动态页面、反爬虫策略等",
		category: "regular",
		author: "WebScraper",
		icon: "spider",
		downloads: 21345,
		rating: 4.8,
		tags: ["爬虫", "数据采集", "网页解析"],
		installed: true,
		version: "3.1.0",
	},
	{
		id: "5",
		name: "漏洞扫描",
		description:
			"自动扫描代码和依赖中的安全漏洞，支持 CVE 数据库查询和修复建议",
		category: "security",
		author: "SecLab",
		icon: "shield",
		downloads: 19876,
		rating: 4.9,
		tags: ["漏洞扫描", "安全审计", "CVE"],
		installed: false,
		version: "2.3.1",
	},
	{
		id: "6",
		name: "密钥管理",
		description: "安全的密钥和凭证管理工具，支持加密存储、自动轮换、访问控制",
		category: "security",
		author: "VaultTeam",
		icon: "key",
		downloads: 16543,
		rating: 4.8,
		tags: ["密钥管理", "加密", "凭证"],
		installed: true,
		version: "1.8.3",
	},
	{
		id: "7",
		name: "代码混淆",
		description:
			"保护 JavaScript 和 TypeScript 代码，防止逆向工程和知识产权泄露",
		category: "security",
		author: "ObfuscatorPro",
		icon: "lock",
		downloads: 14321,
		rating: 4.5,
		tags: ["代码混淆", "保护", "反逆向"],
		installed: false,
		version: "3.0.2",
	},
	{
		id: "8",
		name: "安全审计",
		description:
			"全面的安全审计工具，检测 OWASP Top 10 漏洞、配置错误等安全问题",
		category: "security",
		author: "AuditSec",
		icon: "search",
		downloads: 17890,
		rating: 4.7,
		tags: ["安全审计", "OWASP", "渗透测试"],
		installed: false,
		version: "2.1.5",
	},
];

export function SkillMarketSection() {
	const state = useReactive({
		search: "",
		category: "all",
		sortBy: "popular" as "popular" | "rating" | "recent",
		skills: MOCK_SKILLS,
	});

	const filteredSkills = state.skills.filter((skill) => {
		const matchSearch =
			!state.search ||
			skill.name.toLowerCase().includes(state.search.toLowerCase()) ||
			skill.description.toLowerCase().includes(state.search.toLowerCase()) ||
			skill.tags.some((tag) =>
				tag.toLowerCase().includes(state.search.toLowerCase()),
			);
		const matchCategory =
			state.category === "all" || skill.category === state.category;
		return matchSearch && matchCategory;
	});

	const sortedSkills = [...filteredSkills].sort((a, b) => {
		if (state.sortBy === "popular") return b.downloads - a.downloads;
		if (state.sortBy === "rating") return b.rating - a.rating;
		return 0; // recent - 需要添加时间戳字段
	});

	return (
		<div className="flex flex-col h-full">
			{/* Search and Filters */}
			<div className="flex flex-col gap-3 p-4 border-b">
				<div className="flex gap-2">
					<div className="relative flex-1">
						<Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
						<Input
							placeholder="搜索技能..."
							value={state.search}
							onChange={(e) => (state.search = e.target.value)}
							className="pl-9"
						/>
					</div>
					<Select
						value={state.sortBy}
						onValueChange={(v) =>
							(state.sortBy = v as "popular" | "rating" | "recent")
						}
					>
						<SelectTrigger className="w-[140px]">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="popular">最受欢迎</SelectItem>
							<SelectItem value="rating">评分最高</SelectItem>
							<SelectItem value="recent">最新发布</SelectItem>
						</SelectContent>
					</Select>
				</div>

				{/* Category Filters */}
				<div className="flex gap-2 flex-wrap">
					{CATEGORIES.map((cat) => (
						<Badge
							key={cat.id}
							variant={state.category === cat.id ? "default" : "outline"}
							className={cn(
								"cursor-pointer transition-colors",
								state.category === cat.id
									? "bg-primary text-primary-foreground"
									: "hover:bg-accent",
							)}
							onClick={() => (state.category = cat.id)}
						>
							{cat.label}
						</Badge>
					))}
				</div>
			</div>

			{/* Skill List */}
			<ScrollArea className="flex-1">
				<div className="p-4 grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
					{sortedSkills.map((skill) => (
						<SkillCard key={skill.id} skill={skill} />
					))}
				</div>

				{sortedSkills.length === 0 && (
					<div className="flex flex-col items-center justify-center h-64 text-muted-foreground">
						<Zap className="size-12 mb-4 opacity-20" />
						<p className="text-sm">未找到匹配的技能</p>
					</div>
				)}
			</ScrollArea>
		</div>
	);
}

function SkillCard({ skill }: { skill: Skill }) {
	return (
		<div className="flex flex-col gap-3 p-4 rounded-lg border bg-card hover:shadow-md transition-shadow">
			{/* Header */}
			<div className="flex items-start gap-3">
				<div className="flex items-center justify-center size-12 shrink-0 rounded-lg bg-primary/10">
					<Code2 className="size-6 text-primary" />
				</div>
				<div className="flex-1 min-w-0">
					<h3 className="font-semibold text-sm truncate">{skill.name}</h3>
					<p className="text-xs text-muted-foreground">
						by {skill.author} · v{skill.version}
					</p>
				</div>
				{skill.installed && (
					<Badge variant="secondary" className="shrink-0">
						已安装
					</Badge>
				)}
			</div>

			{/* Description */}
			<p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
				{skill.description}
			</p>

			{/* Tags */}
			<div className="flex gap-1.5 flex-wrap">
				{skill.tags.slice(0, 3).map((tag) => (
					<Badge
						key={tag}
						variant="outline"
						className="text-[10px] px-1.5 py-0"
					>
						{tag}
					</Badge>
				))}
			</div>

			{/* Stats */}
			<div className="flex items-center gap-3 text-xs text-muted-foreground">
				<div className="flex items-center gap-1">
					<Download className="size-3" />
					<span>{skill.downloads.toLocaleString()}</span>
				</div>
				<div className="flex items-center gap-1">
					<Star className="size-3 fill-yellow-400 text-yellow-400" />
					<span>{skill.rating}</span>
				</div>
			</div>

			{/* Actions */}
			<div className="flex gap-2 pt-2 border-t">
				{skill.installed ? (
					<>
						<Button variant="outline" size="sm" className="flex-1">
							<ExternalLink className="size-3 mr-1.5" />
							查看详情
						</Button>
						<Button variant="ghost" size="sm" className="flex-1">
							卸载
						</Button>
					</>
				) : (
					<>
						<Button size="sm" className="flex-1">
							<Download className="size-3 mr-1.5" />
							安装
						</Button>
						<Button variant="outline" size="sm" className="flex-1">
							<ExternalLink className="size-3 mr-1.5" />
							详情
						</Button>
					</>
				)}
			</div>
		</div>
	);
}
