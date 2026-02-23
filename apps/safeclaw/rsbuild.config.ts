import { defineConfig } from "@rsbuild/core";
import { pluginLess } from "@rsbuild/plugin-less";
import { pluginReact } from "@rsbuild/plugin-react";
import path from "path";

const isTauri = !!process.env.TAURI_ENV_PLATFORM;

export default defineConfig({
	html: {
		favicon: path.join(__dirname, "public/logo.svg"),
		template: path.join(__dirname, "index.html"),
	},
	source: {
		alias: {
			"@/": path.join(__dirname, "src"),
		},
		decorators: {
			version: "legacy",
		},
	},
	output: {
		distPath: {
			root: "dist",
		},
	},
	server: {
		port: isTauri ? 1420 : 8888,
		strictPort: isTauri,
		proxy: isTauri
			? undefined
			: {
					"/api/polymarket": {
						target: "https://gamma-api.polymarket.com",
						pathRewrite: { "^/api/polymarket": "" },
						changeOrigin: true,
					},
				},
	},
	plugins: [pluginReact(), pluginLess()],
});
