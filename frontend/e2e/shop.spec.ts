import { expect, test } from '@playwright/test';
import { newPlayer, visit } from './helpers';

const coins = (page: import('@playwright/test').Page) =>
	page
		.locator('#wallet .sr')
		.textContent()
		.then((t) => Number(t?.replace(/\D/g, '')));

test('a short buy button opens the top-up, which then buys the booster', async ({ page }) => {
	await newPlayer(page, 4); // 1000 coins spent: balance 0, pile 4
	await visit(page, '/');
	expect(await coins(page)).toBe(0);

	await page.locator('.shelf .buy').first().click();
	const sheet = page.getByRole('dialog', { name: /top up/i });
	await expect(sheet).toBeVisible();
	await expect(sheet).toContainText('250 more coins');
	// The smallest pack covering the shortfall is preselected
	await expect(sheet.locator('[aria-pressed="true"]')).toContainText('500');

	await sheet.getByRole('button', { name: /get 500 coins and buy/i }).click();
	await expect(sheet).toBeHidden();
	await expect.poll(() => coins(page), { timeout: 5000 }).toBe(250);
	await expect(page.locator('[data-nav="open"] .badge')).toHaveText('5');
});

test('the wallet opens the top-up sheet', async ({ page }) => {
	await newPlayer(page);
	await visit(page, '/');
	await page.locator('#wallet').click();
	const sheet = page.getByRole('dialog', { name: /top up/i });
	await expect(sheet.locator('.coinpack')).toHaveCount(5);
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
});

test('shelf buttons line up whatever the collection name length', async ({ page }) => {
	await newPlayer(page);
	await visit(page, '/');
	const tops = await page.locator('.shelf .buy').evaluateAll((els) => {
		// Group buttons by shelf row (same item top) and compare their tops
		const rows = new Map<number, number[]>();
		for (const el of els) {
			const item = el.closest('.shelf-item')!.getBoundingClientRect().top;
			const key = Math.round(item);
			rows.set(key, [...(rows.get(key) ?? []), Math.round(el.getBoundingClientRect().top)]);
		}
		return [...rows.values()];
	});
	for (const row of tops) expect(new Set(row).size, `buttons in a row: ${row}`).toBe(1);
});
