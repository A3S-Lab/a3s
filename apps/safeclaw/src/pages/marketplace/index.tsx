/**
 * Marketplace Page — unified marketplace for agents and skills.
 * Uses shared SidebarLayout for consistent navigation.
 */
import {
	SidebarLayout,
	type SidebarSection,
} from "@/components/layout/sidebar-layout";
import { Bot, Zap } from "lucide-react";
import { useState } from "react";

import { AgentMarketSection } from "./components/agent-market-section";
import { SkillMarketSection } from "./components/skill-market-section";

type SectionId = "agents" | "skills";

const sections: SidebarSection<SectionId>[] = [
	{ id: "agents", label: "智能体市场", icon: Bot, description: "发现外部智能体" },
	{ id: "skills", label: "技能市场", icon: Zap, description: "扩展智能体能力" },
];

export default function MarketplacePage() {
	const [section, setSection] = useState<SectionId>("agents");
	return (
		<SidebarLayout
			title="市场"
			subtitle="发现和安装智能体与技能"
			sections={sections}
			current={section}
			onChange={setSection}
		>
			{section === "agents" && <AgentMarketSection />}
			{section === "skills" && <SkillMarketSection />}
		</SidebarLayout>
	);
}
