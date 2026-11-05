import { defineConfig, devices } from '@playwright/test';

// Browser tests against the real stack: SvelteKit dev server + API + dev database.
// Servers already running (make dev) are reused. CHROMIUM_PATH points at a system
// Chromium to skip Playwright's own download (e.g. CHROMIUM_PATH=/usr/bin/chromium).
const executablePath = process.env.CHROMIUM_PATH || undefined;

export default defineConfig({
	testDir: 'e2e',
	fullyParallel: true,
	retries: process.env.CI ? 1 : 0,
	reporter: process.env.CI ? 'github' : 'list',
	use: {
		baseURL: 'http://localhost:5173',
		trace: 'retain-on-failure',
		launchOptions: { executablePath }
	},
	projects: [
		{ name: 'phone', use: { ...devices['Pixel 7'], launchOptions: { executablePath } } },
		{
			name: 'desktop',
			use: {
				...devices['Desktop Chrome'],
				viewport: { width: 1440, height: 900 },
				launchOptions: { executablePath }
			}
		}
	],
	webServer: [
		{
			command: 'cargo run -p anycardz-api',
			cwd: '..',
			url: 'http://127.0.0.1:8080/api/health',
			reuseExistingServer: true,
			timeout: 300_000
		},
		{
			// E2E_BUILD=1 (CI) tests the production build: no on-demand compiling,
			// so timing checks hold on slow machines
			command: process.env.E2E_BUILD
				? 'npm run build && npm run preview -- --port 5173 --strictPort'
				: 'npm run dev -- --port 5173 --strictPort',
			url: 'http://localhost:5173/login',
			reuseExistingServer: true,
			timeout: 180_000
		}
	]
});
