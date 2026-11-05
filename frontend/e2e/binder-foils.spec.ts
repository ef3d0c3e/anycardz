import { expect, test } from '@playwright/test';
import { cardOf, give, newPlayer, visit } from './helpers';

// AC-36: every foil showed the same mask, whatever its own pattern
test('the binder and inspector show a foil with its own pattern', async ({ page }) => {
	await newPlayer(page);
	const rare = await cardOf(page, 'ww2-warships', 'rare');
	await give(page, rare.slug, 1, 'foil5');
	await visit(page, '/binder?set=ww2-warships');
	await page.getByRole('button', { name: 'Owned' }).click();
	const card = page.locator('.pocket .card', { hasText: rare.title });
	await expect(card).toHaveAttribute('style', /foil5\.png/);
	await page.locator('.pocket', { hasText: rare.title }).click();
	await expect(page.locator('.ins-card .card')).toHaveAttribute('style', /foil5\.png/);
});
