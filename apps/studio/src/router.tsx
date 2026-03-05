import WorkspaceLayout from "@/layouts/workspace";
import GeneralError from "@/pages/errors/general-error";
import MaintenanceError from "@/pages/errors/maintenance-error";
import NotFoundError from "@/pages/errors/not-found-error";
import UnauthorisedError from "@/pages/errors/unauthorised-error";
import { createHashRouter } from "react-router-dom";

const router = createHashRouter([
	{
		path: "/",
		element: <WorkspaceLayout />,
		children: [
			{
				index: true,
				lazy: async () => ({
					Component: (await import("@/pages/home")).default,
				}),
			},
			{
				path: "settings",
				lazy: async () => ({
					Component: (await import("@/pages/settings")).default,
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
