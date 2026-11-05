import { expect, test } from '@playwright/test';
import { newPlayer, setMissions, visit } from './helpers';

test('daily missions fill up from play, pay, and all three give a free pack', async ({ page }) => {
	const name = await newPlayer(page);
	await setMissions(name, ['free_packs', 'open_packs', 'shop_sell']);
	await visit(page, '/');
	const panel = page.getByRole('region', { name: /0 of 3 done/ });
	await expect(panel.getByRole('listitem')).toHaveCount(3);
	await expect(panel).toContainText('Open 3 packs');
	await expect(panel).toContainText(/New ones in \d\d:\d\d:\d\d/);

	// Claiming the free packs is one of them
	await page.getByRole('button', { name: 'Reveal the packs' }).click();
	await expect(page.getByText('Mission done: Claim your free packs · +100 coins')).toBeVisible();
	await expect(page.getByRole('heading', { name: '1 of 3 done' })).toBeVisible();

	// The other two, through the API
	for (let i = 0; i < 3; i++)
		expect((await page.request.post('/api/pile/open', { data: {} })).ok()).toBe(true);
	const binder = await (await page.request.get('/api/binder')).json();
	// Plain copies: selling without saying foil sells a plain one
	const copies: string[] = binder.cards.flatMap(
		(c: { card: string; count: number; foils: number }) => Array(c.count - c.foils).fill(c.card)
	);
	for (const card of copies.slice(0, 3))
		expect((await page.request.post('/api/binder/sell', { data: { card } })).ok()).toBe(true);
	// Several toasts queue here (first pack, maybe a first legendary): the panel says it at once
	await expect(page.getByRole('heading', { name: '3 of 3 done' })).toBeVisible();
	await expect(page.getByText('Free pack added to your pile')).toBeVisible();
	await expect(page.locator('.pile-stack .pack.free')).toHaveCount(1);
});
