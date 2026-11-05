import { expect, test } from '@playwright/test';
import { newPlayer, reelMode, sql, tearPack, visit } from './helpers';

test('free packs: claimed from the lobby, opened like any other', async ({ page }) => {
	await newPlayer(page);
	await reelMode(page, 'all');
	await visit(page, '/');
	const free = page.getByRole('region', { name: '3 packs' });
	await expect(free.locator('canvas')).toBeVisible();
	await free.getByRole('button', { name: 'Reveal the packs' }).click();
	await expect(page.getByText('3 free packs added to your pile')).toBeVisible();
	await expect(free.getByRole('button', { name: 'Reveal the packs' })).toHaveCount(0);
	await expect(free.getByText('Claimed')).toBeVisible();
	await expect(free.getByText(/^Next packs in 0[01]:\d\d:\d\d$/)).toBeVisible();
	await expect(page.locator('.pile-text h2')).toHaveText('3 packs waiting');
	await expect(page.locator('.pile-stack .pack.free')).toHaveCount(3);

	await visit(page, '/open');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('Free pack');
	const opened = page.waitForResponse((r) => r.url().endsWith('/api/pile/open'));
	await tearPack(page);
	expect((await opened).status()).toBe(200);
	await page.getByRole('button', { name: /skip/i }).click();
	await expect(page.locator('.tray .slotmini.pop')).toHaveCount(5);
});

test('the legendary is guaranteed after enough dry packs', async ({ page }) => {
	const name = await newPlayer(page, 1);
	await sql('UPDATE users SET dry_packs = 14 WHERE username = $1', [name]);
	await reelMode(page, 'all');
	await visit(page, '/open');
	await expect(page.getByText('Legendary guaranteed in this pack')).toBeVisible();
	const opened = page.waitForResponse((r) => r.url().endsWith('/api/pile/open'));
	await tearPack(page);
	const pack = await (await opened).json();
	expect(pack.cards.some((c: { rarity: string }) => c.rarity === 'legendary')).toBe(true);
	expect(pack.pity_left).toBe(15);
});
