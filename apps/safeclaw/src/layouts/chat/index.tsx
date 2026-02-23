import { PageLoading } from "@/components/custom/page-loading";
import OnboardingWizard, {
	isOnboardingComplete,
} from "@/components/custom/onboarding-wizard";
import globalModel from "@/models/global.model";
import KeepAlive, { useKeepAliveRef } from "keepalive-for-react";
import { AnimatePresence, motion } from "motion/react";
import { Suspense, useMemo, useState } from "react";
import { Await, useLocation, useOutlet } from "react-router-dom";
import ActivityBar from "./components/activity-bar";
import Main from "./components/main";

export default function ChatLayout() {
	const aliveRef = useKeepAliveRef();
	const location = useLocation();
	const outlet = useOutlet();
	const [showOnboarding, setShowOnboarding] = useState(
		() => !isOnboardingComplete(),
	);

	const currentCacheKey = useMemo(() => {
		return location.pathname + location.search;
	}, [location.pathname, location.search]);

	return (
		<div className="flex h-screen w-screen bg-secondary">
			<ActivityBar />
			<Main>
				<Suspense
					fallback={
						<div className="flex flex-1 justify-center items-center">
							<PageLoading />
						</div>
					}
				>
					<KeepAlive
						aliveRef={aliveRef}
						activeCacheKey={currentCacheKey}
						transition
						max={5}
					>
						<AnimatePresence>
							<motion.div
								className="flex-1 w-full h-full"
								key={currentCacheKey}
								initial={{ opacity: 0, filter: "blur(8px)" }}
								animate={{ opacity: 1, filter: "blur(0)" }}
								exit={{ opacity: 0, filter: "blur(8px)" }}
								transition={{ duration: 0.25, ease: "easeInOut" }}
							>
								<Await resolve={globalModel.load()}>
									{() => {
										return outlet;
									}}
								</Await>
							</motion.div>
						</AnimatePresence>
					</KeepAlive>
				</Suspense>
			</Main>
			{showOnboarding && (
				<OnboardingWizard onComplete={() => setShowOnboarding(false)} />
			)}
		</div>
	);
}
