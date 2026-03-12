import { defineConfig } from "@rsbuild/core";
import { pluginLess } from "@rsbuild/plugin-less";
import { pluginSass } from "@rsbuild/plugin-sass";
import { pluginReact } from "@rsbuild/plugin-react";
import path from "path";

const isTauri = !!process.env.TAURI_ENV_PLATFORM;

export default defineConfig({
	html: {
		favicon: path.join(__dirname, "public/logo.png"),
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
	},
	plugins: [pluginReact(), pluginLess(), pluginSass()],
});
