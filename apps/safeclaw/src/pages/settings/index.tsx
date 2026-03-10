/**
 * Settings Page — unified settings dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import { Bot, FolderOpen, Info } from "lucide-react";
import { useState } from "react";

import { AboutSection } from "./components/about-section";
import { AiSection } from "./components/ai-section";
import { WorkspaceSection } from "./components/workspace-section";

type SectionId = "workspace" | "ai" | "about";

const sections: SidebarSection<SectionId>[] = [
	{
		id: "workspace",
		label: "工作区",
		icon: FolderOpen,
		description: "目录与会话",
	},
	{ id: "ai", label: "AI 服务", icon: Bot, description: "模型与认证" },
	{ id: "about", label: "关于", icon: Info, description: "版本与数据" },
];

export default function SettingsPage() {
	const [section, setSection] = useState<SectionId>("workspace");
	return (
		<SidebarLayout
			title="设置"
			subtitle="管理应用配置"
			sections={sections}
			current={section}
			onChange={setSection}
		>
			{section === "workspace" && <WorkspaceSection />}
			{section === "ai" && <AiSection />}
			{section === "about" && <AboutSection />}
		</SidebarLayout>
	);
}
