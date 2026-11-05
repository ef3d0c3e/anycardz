import { expect, test } from '@playwright/test';
import { cardOf, newPlayer, visit } from './helpers';

const palette = (page: import('@playwright/test').Page) =>
	page.getByRole('dialog', { name: /search cards, sets, players and pages/i });

test('Ctrl+K finds a card and opens it in the binder', async ({ page }) => {
	await newPlayer(page);
	const card = await cardOf(page, 'ww2-warships', 'legendary');
	await visit(page, '/');
	await page.keyboard.press('Control+k');
	await expect(palette(page)).toBeVisible();
	await page.keyboard.type(card.title.slice(0, 6));
	await expect(palette(page).getByRole('option', { name: card.title })).toBeVisible();
	await palette(page).getByRole('option', { name: card.title }).click();
	await expect(page).toHaveURL(/\/binder\?set=ww2-warships&card=/);
	await expect(page.getByRole('dialog', { name: /unknown card/i })).toBeVisible();
});

test('arrow keys and Enter pick a player', async ({ page, browser }) => {
	const other = await (await browser.newContext()).newPage();
	const name = await newPlayer(other);
	await newPlayer(page);
	await visit(page, '/forge');
	await page.getByRole('button', { name: /search cards, sets, players and pages/i }).click();
	await page.keyboard.type(name);
	const option = palette(page).getByRole('option', { name });
	await expect(option).toBeVisible();
	// Pages come first: walk down to the player
	for (let i = 0; i < 10 && (await option.getAttribute('aria-selected')) !== 'true'; i++)
		await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(page).toHaveURL(new RegExp(`/players/${name}$`));
	await expect(page.locator('h1')).toHaveText(name);
});
