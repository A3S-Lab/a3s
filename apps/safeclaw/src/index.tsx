import { ModalProvider } from "@/components/custom/modal-provider";
import { ThemeProvider } from "@/components/custom/theme-provider";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import { Agentation } from "agentation";
import constants from "@/constants";
import settingsModel from "@/models/settings.model";
import router from "./router";

import "dayjs/locale/zh-cn";
import "./index.css";

dayjs.locale("zh-cn");
dayjs.extend(relativeTime);

// Seed settings from backend config on first launch (no-op if localStorage exists)
settingsModel.seedFromBackend();

const rootEl = document.getElementById("root");
if (rootEl) {
	const root = ReactDOM.createRoot(rootEl);
	root.render(
		<ThemeProvider>
			<ModalProvider>
				<TooltipProvider>
					<RouterProvider router={router} />
					<Toaster position="top-right" duration={3000} />
					{constants.isDev && <Agentation />}
				</TooltipProvider>
			</ModalProvider>
		</ThemeProvider>,
	);
}
