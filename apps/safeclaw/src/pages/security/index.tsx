/**
 * Security Center — unified security dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import securityModel from "@/models/security.model";
import {
	Bug,
	Fingerprint,
	FileWarning,
	Globe,
	MessageSquare,
	Shield,
} from "lucide-react";
import { useState } from "react";
import { useSnapshot } from "valtio";

import { OverviewSection } from "./components/overview-section";
import { AuditSection } from "./components/audit-section";
import { PrivacySection } from "./components/privacy-section";
import { FirewallSection } from "./components/firewall-section";
import { TaintSection } from "./components/taint-section";
import { ChannelsSection } from "./components/channels-section";

type SectionId =
	| "overview"
	| "audit"
	| "privacy"
	| "firewall"
	| "taint"
	| "channels";

const sections: SidebarSection<SectionId>[] = [
	{
		id: "overview",
		label: "安全概览",
		icon: Shield,
		description: "总览与指标",
	},
	{
		id: "audit",
		label: "审计日志",
		icon: FileWarning,
		description: "安全事件",
	},
	{
		id: "privacy",
		label: "隐私扫描",
		icon: Fingerprint,
		description: "PII 检测",
	},
	{
		id: "channels",
		label: "渠道管理",
		icon: MessageSquare,
		description: "消息渠道",
	},
	{
		id: "firewall",
		label: "网络防火墙",
		icon: Globe,
		description: "出站白名单",
	},
	{ id: "taint", label: "污点追踪", icon: Bug, description: "数据流标记" },
];

export default function SecurityPage() {
	const [section, setSection] = useState<SectionId>("overview");
	const snap = useSnapshot(securityModel.state);

	return (
		<SidebarLayout
			title="安全中心"
			subtitle="监控与防护"
			sections={sections}
			current={section}
			onChange={setSection}
			contentMaxWidth="max-w-4xl"
			footer="SafeClaw Security · a3s-code v0.9.0"
			badge={(id) =>
				id === "audit" && snap.alerts.length > 0 ? (
					<span className="text-[10px] rounded-full px-1.5 py-0.5 min-w-[20px] text-center bg-destructive/20 text-destructive font-medium">
						{snap.alerts.length}
					</span>
				) : null
			}
		>
			{section === "overview" && <OverviewSection />}
			{section === "audit" && <AuditSection />}
			{section === "privacy" && <PrivacySection />}
			{section === "channels" && <ChannelsSection />}
			{section === "firewall" && <FirewallSection />}
			{section === "taint" && <TaintSection />}
		</SidebarLayout>
	);
}
