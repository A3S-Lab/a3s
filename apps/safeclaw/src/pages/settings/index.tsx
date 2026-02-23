/**
 * Settings Page — unified settings dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import { Bot, Cpu, Info, Server, ShieldCheck } from "lucide-react";
import { useState } from "react";

import { AiSection } from "./components/ai-section";
import { AgentSection } from "./components/agent-section";
import { GatewaySection } from "./components/gateway-section";
import { DiagnosticsSection } from "./components/diagnostics-section";
import { AboutSection } from "./components/about-section";

type SectionId = "ai" | "agent" | "gateway" | "diagnostics" | "about";

const sections: SidebarSection<SectionId>[] = [
	{ id: "ai", label: "AI 服务", icon: Bot, description: "模型与认证" },
	{ id: "agent", label: "Agent 配置", icon: Cpu, description: "行为与默认值" },
	{ id: "gateway", label: "网关连接", icon: Server, description: "服务地址" },
	{
		id: "diagnostics",
		label: "诊断",
		icon: ShieldCheck,
		description: "系统健康",
	},
	{ id: "about", label: "关于", icon: Info, description: "版本与数据" },
];

export default function SettingsPage() {
	const [section, setSection] = useState<SectionId>("ai");
	return (
		<SidebarLayout
			title="设置"
			subtitle="管理应用配置"
			sections={sections}
			current={section}
			onChange={setSection}
		>
			{section === "ai" && <AiSection />}
			{section === "agent" && <AgentSection />}
			{section === "gateway" && <GatewaySection />}
			{section === "diagnostics" && <DiagnosticsSection />}
			{section === "about" && <AboutSection />}
		</SidebarLayout>
	);
}
