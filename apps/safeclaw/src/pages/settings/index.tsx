/**
 * Settings Page — unified settings dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import { Bot, Info, PlugZap, ShieldCheck } from "lucide-react";
import { useState } from "react";

import { AboutSection } from "./components/about-section";
import { AiSection } from "./components/ai-section";
import { DiagnosticsSection } from "./components/diagnostics-section";
import { McpSection } from "./components/mcp-section";

type SectionId = "ai" | "mcp" | "diagnostics" | "about";

const sections: SidebarSection<SectionId>[] = [
	{ id: "ai", label: "AI 服务", icon: Bot, description: "模型与认证" },
	{ id: "mcp", label: "MCP 服务", icon: PlugZap, description: "全局服务配置" },
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
			{section === "mcp" && <McpSection />}
			{section === "diagnostics" && <DiagnosticsSection />}
			{section === "about" && <AboutSection />}
		</SidebarLayout>
	);
}
