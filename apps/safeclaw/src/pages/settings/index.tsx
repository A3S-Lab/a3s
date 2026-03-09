/**
 * Settings Page — unified settings dashboard.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import { FolderOpen, Info } from "lucide-react";
import { useState } from "react";

import { AboutSection } from "./components/about-section";
import { WorkspaceSection } from "./components/workspace-section";

type SectionId = "workspace" | "about";

const sections: SidebarSection<SectionId>[] = [
	{
		id: "workspace",
		label: "工作区",
		icon: FolderOpen,
		description: "目录与会话",
	},
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
			{section === "about" && <AboutSection />}
		</SidebarLayout>
	);
}
