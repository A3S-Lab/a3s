import { useState } from "react";
import {
	FileText,
	Upload,
	AlertTriangle,
	CheckCircle,
	Clock,
	ChevronDown,
	Shield,
} from "lucide-react";
import { cn } from "@/lib/utils";

type RiskLevel = "high" | "medium" | "low";

interface Clause {
	id: string;
	title: string;
	content: string;
	risk: RiskLevel;
	suggestion: string;
	article: string;
}

interface Contract {
	id: string;
	name: string;
	party: string;
	type: string;
	uploadedAt: number;
	status: "analyzing" | "done";
	clauses: Clause[];
	score: number;
}

const RISK_STYLES: Record<RiskLevel, string> = {
	high: "bg-red-100 text-red-700 border-red-200 dark:bg-red-900/30 dark:text-red-400 dark:border-red-800",
	medium:
		"bg-yellow-100 text-yellow-700 border-yellow-200 dark:bg-yellow-900/30 dark:text-yellow-400 dark:border-yellow-800",
	low: "bg-green-100 text-green-700 border-green-200 dark:bg-green-900/30 dark:text-green-400 dark:border-green-800",
};

const RISK_LABELS: Record<RiskLevel, string> = {
	high: "高风险",
	medium: "中风险",
	low: "低风险",
};

const RISK_ICONS: Record<RiskLevel, typeof AlertTriangle> = {
	high: AlertTriangle,
	medium: Clock,
	low: CheckCircle,
};

const MOCK_CONTRACTS: Contract[] = [
	{
		id: "1",
		name: "软件开发服务合同.pdf",
		party: "XX科技有限公司",
		type: "服务合同",
		uploadedAt: Date.now() - 3600000,
		status: "done",
		score: 72,
		clauses: [
			{
				id: "c1",
				article: "第5条",
				title: "知识产权归属",
				content:
					"乙方在履行本合同过程中产生的所有知识产权归甲方所有，乙方不得主张任何权利。",
				risk: "high",
				suggestion:
					"该条款过于宽泛，建议明确区分甲方提供的背景知识产权与乙方在本项目中新创作的成果，避免乙方既有技术被无偿转让。",
			},
			{
				id: "c2",
				article: "第8条",
				title: "违约责任",
				content: "任何一方违约，应向守约方支付合同总价款20%的违约金。",
				risk: "medium",
				suggestion:
					"违约金比例偏高，建议参照实际损失设定上限，或区分不同违约情形设置差异化违约金标准。",
			},
			{
				id: "c3",
				article: "第12条",
				title: "保密义务",
				content:
					"双方对本合同内容及履行过程中获知的对方商业秘密负有保密义务，保密期限为合同终止后3年。",
				risk: "low",
				suggestion:
					"条款合理，建议补充明确保密信息的范围定义，以避免后续争议。",
			},
			{
				id: "c4",
				article: "第15条",
				title: "争议解决",
				content:
					"因本合同引起的争议，双方协商解决；协商不成，提交甲方所在地仲裁委员会仲裁。",
				risk: "medium",
				suggestion:
					"仲裁地点对乙方不利，建议协商改为双方共同认可的第三方仲裁机构，或约定诉讼管辖法院。",
			},
		],
	},
	{
		id: "2",
		name: "战略合作框架协议.docx",
		party: "YY集团",
		type: "合作协议",
		uploadedAt: Date.now() - 86400000,
		status: "done",
		score: 88,
		clauses: [
			{
				id: "c5",
				article: "第3条",
				title: "排他性条款",
				content: "合作期间，甲方不得与乙方竞争对手开展同类业务合作。",
				risk: "high",
				suggestion:
					"排他性约束范围过广，建议明确界定「竞争对手」和「同类业务」的具体范围，并设定合理的地域和时间限制。",
			},
			{
				id: "c6",
				article: "第7条",
				title: "收益分配",
				content: "合作产生的收益按照甲方60%、乙方40%的比例分配。",
				risk: "low",
				suggestion: "分配比例明确，建议补充收益计算方式和结算周期的具体约定。",
			},
		],
	},
];

function ScoreRing({ score }: { score: number }) {
	const color =
		score >= 80
			? "text-green-500"
			: score >= 60
				? "text-yellow-500"
				: "text-red-500";
	return (
		<div className={cn("text-2xl font-bold tabular-nums", color)}>
			{score}
			<span className="text-xs font-normal text-muted-foreground ml-0.5">
				分
			</span>
		</div>
	);
}

export default function ContractReviewPage() {
	const [contracts] = useState<Contract[]>(MOCK_CONTRACTS);
	const [selected, setSelected] = useState<Contract>(MOCK_CONTRACTS[0]);
	const [expanded, setExpanded] = useState<string | null>(null);
	const [filterRisk, setFilterRisk] = useState<RiskLevel | "all">("all");

	const filteredClauses = selected.clauses.filter(
		(c) => filterRisk === "all" || c.risk === filterRisk,
	);

	const riskCounts = {
		high: selected.clauses.filter((c) => c.risk === "high").length,
		medium: selected.clauses.filter((c) => c.risk === "medium").length,
		low: selected.clauses.filter((c) => c.risk === "low").length,
	};

	return (
		<div className="flex flex-col h-screen bg-background text-foreground">
			{/* Header */}
			<div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-background/95 backdrop-blur shrink-0">
				<div className="size-8 rounded-xl bg-gradient-to-br from-purple-500 to-purple-600 flex items-center justify-center">
					<FileText className="size-4 text-white" />
				</div>
				<div>
					<h1 className="text-sm font-semibold leading-none">合同审查</h1>
					<p className="text-[10px] text-muted-foreground mt-0.5">
						AI 风险识别与条款分析
					</p>
				</div>
				<div className="flex-1" />
				<button
					type="button"
					className="flex items-center gap-1.5 text-xs bg-primary text-primary-foreground px-3 py-1.5 rounded-lg hover:bg-primary/90 transition-colors"
				>
					<Upload className="size-3" />
					上传合同
				</button>
			</div>

			{/* Contract tabs */}
			<div className="flex gap-1 px-3 py-2 border-b border-border overflow-x-auto shrink-0">
				{contracts.map((c) => (
					<button
						key={c.id}
						type="button"
						onClick={() => setSelected(c)}
						className={cn(
							"flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-lg whitespace-nowrap transition-colors shrink-0",
							selected.id === c.id
								? "bg-primary text-primary-foreground"
								: "bg-muted text-muted-foreground hover:bg-muted/80",
						)}
					>
						<FileText className="size-3" />
						{c.name.replace(/\.(pdf|docx)$/, "")}
					</button>
				))}
			</div>

			<div className="flex-1 overflow-y-auto">
				{/* Contract overview */}
				<div className="px-4 py-3 border-b border-border">
					<div className="flex items-start justify-between mb-3">
						<div>
							<p className="text-sm font-medium">{selected.name}</p>
							<p className="text-xs text-muted-foreground mt-0.5">
								{selected.party} · {selected.type}
							</p>
						</div>
						<ScoreRing score={selected.score} />
					</div>

					{/* Risk summary */}
					<div className="grid grid-cols-3 gap-2">
						{(["high", "medium", "low"] as RiskLevel[]).map((risk) => {
							const Icon = RISK_ICONS[risk];
							return (
								<button
									key={risk}
									type="button"
									onClick={() =>
										setFilterRisk(filterRisk === risk ? "all" : risk)
									}
									className={cn(
										"flex flex-col items-center gap-1 p-2 rounded-xl border transition-all",
										filterRisk === risk
											? RISK_STYLES[risk]
											: "border-border bg-muted/30 text-muted-foreground hover:bg-muted/60",
									)}
								>
									<Icon className="size-4" />
									<span className="text-lg font-bold tabular-nums">
										{riskCounts[risk]}
									</span>
									<span className="text-[10px]">{RISK_LABELS[risk]}</span>
								</button>
							);
						})}
					</div>
				</div>

				{/* Clauses */}
				<div className="divide-y divide-border">
					{filteredClauses.length === 0 ? (
						<div className="flex flex-col items-center justify-center py-12 text-muted-foreground gap-2">
							<Shield className="size-8 opacity-30" />
							<p className="text-sm">暂无风险条款</p>
						</div>
					) : (
						filteredClauses.map((clause) => {
							const Icon = RISK_ICONS[clause.risk];
							const isOpen = expanded === clause.id;
							return (
								<div key={clause.id} className="px-4 py-3">
									<button
										type="button"
										className="w-full text-left"
										onClick={() => setExpanded(isOpen ? null : clause.id)}
									>
										<div className="flex items-center gap-2">
											<span
												className={cn(
													"text-[10px] px-1.5 py-0.5 rounded-full border font-medium",
													RISK_STYLES[clause.risk],
												)}
											>
												{RISK_LABELS[clause.risk]}
											</span>
											<span className="text-xs text-muted-foreground">
												{clause.article}
											</span>
											<span className="text-xs font-medium flex-1">
												{clause.title}
											</span>
											<ChevronDown
												className={cn(
													"size-3.5 text-muted-foreground transition-transform shrink-0",
													isOpen && "rotate-180",
												)}
											/>
										</div>
									</button>

									{isOpen && (
										<div className="mt-2.5 space-y-2">
											{/* Original clause */}
											<div className="p-2.5 bg-muted/50 rounded-lg">
												<p className="text-[10px] text-muted-foreground mb-1 font-medium">
													原文
												</p>
												<p className="text-xs leading-relaxed">
													{clause.content}
												</p>
											</div>
											{/* AI suggestion */}
											<div
												className={cn(
													"p-2.5 rounded-lg border",
													RISK_STYLES[clause.risk],
												)}
											>
												<div className="flex items-center gap-1 mb-1">
													<Icon className="size-3" />
													<p className="text-[10px] font-medium">AI 建议</p>
												</div>
												<p className="text-xs leading-relaxed">
													{clause.suggestion}
												</p>
											</div>
										</div>
									)}
								</div>
							);
						})
					)}
				</div>
			</div>

			{/* Footer */}
			<div className="px-4 py-2 border-t border-border bg-muted/30 shrink-0">
				<p className="text-[10px] text-muted-foreground text-center">
					共 {selected.clauses.length} 条条款 · {riskCounts.high} 高风险 · AI
					分析完成
				</p>
			</div>
		</div>
	);
}
