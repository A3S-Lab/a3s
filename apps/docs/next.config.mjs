import { createMDX } from 'fumadocs-mdx/next';

const isGitHubPages = process.env.GITHUB_ACTIONS === 'true';

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  output: 'export',
  basePath: isGitHubPages ? '/a3s' : '',
  images: { unoptimized: true },
};

const withMDX = createMDX();
export default withMDX(config);
