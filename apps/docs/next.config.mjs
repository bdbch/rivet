import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();
const basePath = process.env.GITHUB_ACTIONS === 'true' ? '/rivet' : undefined;

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  basePath,
  reactStrictMode: true,
};

export default withMDX(config);
