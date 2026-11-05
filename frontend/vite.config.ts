import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

// The API (backend/) listens here in development
const api = process.env.API_URL ?? 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter()
		})
	],
	test: {
		// Unit tests next to the code; browser tests live in e2e/ (Playwright)
		include: ['src/**/*.test.ts']
	},
	server: {
		// Browser calls to /api reach the API with their cookies, same origin
		proxy: { '/api': { target: api, changeOrigin: false } }
	}
});
