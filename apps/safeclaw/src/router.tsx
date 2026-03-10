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
				path: "repos/:id",
				lazy: async () => ({
					Component: (await import("@/pages/repos/editor")).default,
				}),
			},
			{
				path: "box",
				lazy: async () => ({
					Component: (await import("@/pages/box")).default,
				}),
			},
			{
				path: "workflow",
				lazy: async () => ({
					Component: (await import("@/pages/workflow")).default,
				}),
			},
			{
				path: "workflow/:id",
				lazy: async () => ({
					Component: (await import("@/pages/workflow/editor")).default,
				}),
			},
			{
				path: "marketplace",
				lazy: async () => ({
					Component: (await import("@/pages/marketplace")).default,
				}),
			},
			{
				path: "agent-marketplace",
				lazy: async () => ({
					Component: (await import("@/pages/agent-marketplace")).default,
				}),
			},
			{
				path: "skill-marketplace",
				lazy: async () => ({
					Component: (await import("@/pages/skill-marketplace")).default,
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
	{
		path: "/builtin/:id",
		lazy: async () => ({
			Component: (await import("@/pages/builtin")).default,
		}),
	},
	{
		path: "/onboarding",
		lazy: async () => ({
			Component: (await import("@/pages/onboarding")).default,
		}),
	},
	{ path: "/500", Component: GeneralError },
	{ path: "/404", Component: NotFoundError },
	{ path: "/503", Component: MaintenanceError },
	{ path: "/401", Component: UnauthorisedError },
	{ path: "*", Component: NotFoundError },
]);

export default router;
