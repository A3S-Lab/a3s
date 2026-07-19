import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

const apiOrigin = process.env.A3S_BENCH_API_ORIGIN ?? 'http://127.0.0.1:29655';

export default defineConfig({
  plugins: [pluginReact()],
  source: {
    entry: {
      index: './src/main.tsx',
    },
  },
  html: {
    template: './index.html',
    title: 'A3S智能体评测',
    meta: {
      'application-name': 'A3S智能体评测',
      description: 'A3S智能体评测可视化工作台',
      'theme-color': '#071014',
    },
  },
  output: {
    cleanDistPath: true,
    distPath: {
      root: 'dist/windhole',
    },
    assetPrefix: '/',
  },
  server: {
    port: Number(process.env.A3S_WINDHOLE_DEV_PORT ?? 3030),
    proxy: {
      '/api': {
        target: apiOrigin,
        changeOrigin: true,
      },
    },
  },
});
