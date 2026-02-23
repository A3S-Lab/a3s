import ChatLayout from "@/layouts/chat";
import GeneralError from "@/pages/errors/general-error";
import MaintenanceError from "@/pages/errors/maintenance-error";
import NotFoundError from "@/pages/errors/not-found-error";
import UnauthorisedError from "@/pages/errors/unauthorised-error";
import { createHashRouter } from "react-router-dom";

const router = createHashRouter([
	{
		path: "/",
		element: <ChatLayout />,
		children: [
			{
				index: true,
				lazy: async () => ({
					Component: (await import("@/pages/agent")).default,
				}),
			},
			{
				path: "settings",
				lazy: async () => ({
					Component: (await import("@/pages/settings")).default,
				}),
			},
			{
				path: "security",
				lazy: async () => ({
					Component: (await import("@/pages/security")).default,
				}),
			},
			{
				path: "memory",
				lazy: async () => ({
					Component: (await import("@/pages/memory")).default,
				}),
			},
			{
				path: "box",
				lazy: async () => ({
					Component: (await import("@/pages/box")).default,
				}),
			},
		],
	},
	{
		path: "/agent/:agentId",
		element: <ChatLayout />,
		children: [
			{
				index: true,
				lazy: async () => ({
					Component: (await import("@/pages/agent-detail")).default,
				}),
			},
		],
	},
	{ path: "/500", Component: GeneralError },
	{ path: "/404", Component: NotFoundError },
	{ path: "/503", Component: MaintenanceError },
	{ path: "/401", Component: UnauthorisedError },
	{ path: "*", Component: NotFoundError },
]);

export default router;
