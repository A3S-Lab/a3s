import { PageLoading } from "@/components/custom/page-loading";
import globalModel from "@/models/global.model";
import KeepAlive, { useKeepAliveRef } from "keepalive-for-react";
import { AnimatePresence, motion } from "motion/react";
import { Suspense, useMemo } from "react";
import { Navigate, useLocation, useOutlet } from "react-router-dom";
import { useSnapshot } from "valtio";
import ActivityBar from "./components/activity-bar";
import Main from "./components/main";

export default function ChatLayout() {
	const { isOnboarded } = useSnapshot(globalModel.state);
	const aliveRef = useKeepAliveRef();
	const location = useLocation();
	const outlet = useOutlet();

	if (!isOnboarded) {
		return <Navigate to="/onboarding" replace />;
	}

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
								{outlet}
							</motion.div>
						</AnimatePresence>
					</KeepAlive>
				</Suspense>
			</Main>
		</div>
	);
}
