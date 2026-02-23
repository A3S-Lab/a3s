import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import securityModel from "@/models/security.model";
import { Globe, Lock, Network, ShieldCheck, Trash2 } from "lucide-react";
import { useState } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import { SectionHeader } from "./shared";

export function FirewallSection() {
	const snap = useSnapshot(securityModel.state);
	const [newDomain, setNewDomain] = useState("");

	const handleAdd = () => {
		const d = newDomain.trim();
		if (!d) return;
		if (snap.firewallRules.includes(d)) {
			toast.error(`"${d}" 已存在`);
			return;
		}
		securityModel.addFirewallRule(d);
		setNewDomain("");
		toast.success(`已添加 ${d}`);
	};

	return (
		<div>
			<SectionHeader
				icon={Globe}
				title="网络防火墙"
				description="管理出站连接白名单，未列入的域名将被拦截。"
			/>

			<div className="flex items-center gap-2 rounded-lg bg-muted/40 border px-3 py-2 mb-4 text-[11px] text-muted-foreground">
				<Lock className="size-3 shrink-0" />
				<span>
					持久化配置请编辑 <code className="font-mono">safeclaw.hcl</code>
					，此处修改仅影响当前运行时。
				</span>
			</div>

			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="text-sm font-semibold mb-3">添加白名单域名</div>
				<div className="flex items-center gap-2">
					<div className="relative flex-1">
						<Network className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
						<Input
							className="h-9 text-sm font-mono pl-8"
							placeholder="api.example.com"
							value={newDomain}
							onChange={(e) => setNewDomain(e.target.value)}
							onKeyDown={(e) => {
								if (e.key === "Enter") handleAdd();
							}}
						/>
					</div>
					<Button size="sm" onClick={handleAdd} disabled={!newDomain.trim()}>
						添加
					</Button>
				</div>
				<p className="text-[11px] text-muted-foreground mt-2">
					支持通配符，如 *.azure.com
				</p>
			</div>

			<div className="rounded-xl border bg-card p-5">
				<div className="flex items-center gap-2 mb-4">
					<ShieldCheck className="size-4 text-primary" />
					<span className="text-sm font-semibold">白名单规则</span>
					<span className="text-[10px] rounded-full px-1.5 py-0.5 bg-primary/20 text-primary font-medium">
						{snap.firewallRules.length}
					</span>
				</div>
				<div className="rounded-lg bg-muted/30 divide-y divide-border/50">
					{snap.firewallRules.map((rule) => (
						<div
							key={rule}
							className="flex items-center justify-between px-4 py-2.5 group"
						>
							<div className="flex items-center gap-2">
								<Globe className="size-3.5 text-muted-foreground" />
								<span className="text-xs font-mono font-medium">{rule}</span>
							</div>
							<button
								type="button"
								className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-all p-1"
								onClick={() => {
									securityModel.removeFirewallRule(rule);
									toast.success(`已移除 ${rule}`);
								}}
							>
								<Trash2 className="size-3" />
							</button>
						</div>
					))}
				</div>
				<p className="text-[11px] text-muted-foreground mt-3">
					默认策略: 拒绝所有未列入白名单的出站连接。仅允许 LLM API 端点。
				</p>
			</div>
		</div>
	);
}
