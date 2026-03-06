/**
 * Agent Marketplace Page — discover and install external agents.
 */
import { AgentMarketSection } from "./components/agent-market-section";

export default function AgentMarketplacePage() {
	return (
		<div className="flex flex-col h-full">
			<AgentMarketSection />
		</div>
	);
}
