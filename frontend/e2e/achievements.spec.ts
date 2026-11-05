import { expect, test } from '@playwright/test';
import { newPlayer, visit } from './helpers';

test('achievements: earned from play, listed with progress', async ({ page }) => {
	await newPlayer(page, 1, 'desserts');
	await visit(page, '/achievements');
	const first = page.locator('.ach', { hasText: 'First tear' });
	await expect(first).not.toHaveClass(/done/);
	await expect(page.locator('.ach', { hasText: 'Pack rat' })).toContainText('0 / 50');

	await page.request.post('/api/pile/open', { data: {} });
	await visit(page, '/achievements');
	await expect(first).toHaveClass(/done/);
	await expect(first).toContainText(/Earned/);
	await expect(page.locator('.ach', { hasText: 'Pack rat' })).toContainText('1 / 50');
	await expect(page.locator('.summary')).toContainText(/^[1-9]\d* of 18 earned/);
});

test('leaderboards: boards, periods, and the player highlighted', async ({ page }) => {
	const name = await newPlayer(page, 1, 'desserts');
	await page.request.post('/api/pile/open', { data: {} });
	await visit(page, '/leaderboards');
	await expect(page.getByRole('link', { name: 'Sets', exact: true })).toHaveAttribute(
		'aria-current',
		'page'
	);
	await page.getByRole('link', { name: 'Achievements', exact: true }).click();
	await page.getByRole('link', { name: 'This week' }).click();
	await expect(page).toHaveURL(/board=achievements&period=week/);
	// A fresh player with a first pack is on this week's achievements board
	const me = page.locator('.row.me');
	await expect(me).toContainText(name);
	await expect(me).toContainText('You');

	// Streaks have no weekly board
	await page.getByRole('link', { name: 'Streak', exact: true }).click();
	await expect(page).toHaveURL(/board=streak$/);
	await expect(page.getByText('Streaks are all time only.')).toBeVisible();
});

test('an achievement notification leads to that achievement', async ({ page }) => {
	await newPlayer(page, 1, 'desserts');
	await page.request.post('/api/pile/open', { data: {} });
	await visit(page, '/');
	await page.getByRole('button', { name: /unread notification/ }).click();
	await page.getByRole('link', { name: /First tear/ }).click();
	await expect(page).toHaveURL(/\/achievements#ach-first_pack$/);
	const target = page.locator('#ach-first_pack');
	await expect(target).toBeInViewport();
	await expect(target).toContainText('First tear');
});
