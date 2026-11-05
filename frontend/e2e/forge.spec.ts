import { expect, test } from '@playwright/test';
import { cardOf, give, hold, newPlayer, visit } from './helpers';

test('a trade-up turns ten spare commons into a rare', async ({ page }) => {
	await newPlayer(page);
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, common.slug, 11); // 10 spare, 1 kept
	await visit(page, '/forge');

	await expect(page.locator('.tray .tcard')).toHaveCount(1);
	await page.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await expect(page.locator('.core-in b')).toHaveText('10/10');
	await expect(page.locator('.odds li')).toContainText('100%');
	// Fill took the 10 duplicates; the 11th copy stays, marked as the last one
	await expect(page.locator('.tray .c-badge').first()).toHaveText('×1');
	await expect(page.locator('.tray .c-badge.last')).toHaveText('Last copy');

	await hold(page, page.locator('.fuse'), 1800);
	// AC-50: the forged card shows full screen first
	const reveal = page.getByRole('dialog').filter({ hasText: 'Forged' });
	await expect(reveal).toBeVisible({ timeout: 5000 });
	await expect(reveal.locator('.card.r-rare')).toBeVisible();
	await reveal.getByRole('button', { name: 'Continue' }).click();
	await expect(reveal).toBeHidden();
	await expect(page.locator('.core-card .card.r-rare')).toBeVisible();
});

test('letting go of the fuse early keeps the cards', async ({ page }) => {
	await newPlayer(page);
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, common.slug, 11);
	await visit(page, '/forge');
	await page.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await hold(page, page.locator('.fuse'), 500);
	await page.waitForTimeout(600);
	await expect(page.locator('.core-in b')).toHaveText('10/10');
});

test('the roulette shows its odds, then wins a foil or destroys the card', async ({ page }) => {
	await newPlayer(page);
	const legendary = await cardOf(page, 'ww2-warships', 'legendary');
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, legendary.slug, 1);
	await give(page, common.slug, 3);
	await visit(page, '/forge?tab=roulette');

	// The rarest card is picked first; its base odds are 5%
	await expect(page.locator('h1')).toContainText(legendary.title);
	await expect(page.locator('.math')).toContainText('5%');
	await page.locator('.tray .tcard').first().click();
	await expect(page.locator('.math')).toContainText('5.5%');
	await expect(page.locator('.math')).toContainText('+ 1 burned');

	await hold(page, page.locator('.spin'), 1100);
	const banner = page.getByRole('status');
	await expect(banner).toBeVisible({ timeout: 8000 });
	const text = await banner.textContent();
	const reveal = page.getByRole('dialog').filter({ hasText: 'Foil won' });
	if (/now foil/.test(text!)) {
		// AC-50: a won foil shows full screen
		await expect(reveal).toBeVisible();
		await expect(reveal.locator('.holo')).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(page.locator('.dial-target .holo')).toBeVisible();
	} else {
		await expect(reveal).toHaveCount(0);
		expect(text).toMatch(/destroyed/);
		await expect(page.locator('.dial-target.burnt')).toBeVisible();
	}
	// Either way, the burned common is gone from the tray counts
	const binder = await (await page.request.get('/api/binder')).json();
	expect(binder.cards.find((c: { card: string }) => c.card === common.slug).count).toBe(2);
});

// AC-45: any card can burn, not just spare duplicates
test('the roulette can burn a last copy, and says so', async ({ page }) => {
	await newPlayer(page);
	const legendary = await cardOf(page, 'ww2-warships', 'legendary');
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, legendary.slug, 1);
	await give(page, common.slug, 1);
	await visit(page, '/forge?tab=roulette');
	const single = page.locator('.tray .tcard', { hasText: common.title });
	await expect(single.locator('.c-badge.last')).toHaveText('Last copy');
	await single.click();
	await expect(page.locator('.burns .tcard')).toHaveCount(1);
	await hold(page, page.locator('.spin'), 1100);
	await expect(page.getByRole('status')).toBeVisible({ timeout: 8000 });
	const binder = await (await page.request.get('/api/binder')).json();
	expect(binder.cards.find((c: { card: string }) => c.card === common.slug)).toBeUndefined();
});

// AC-46: the trays filter by collection with the binder's picker
test('the trade-up tray filters by set', async ({ page }) => {
	await newPlayer(page);
	const ship = await cardOf(page, 'ww2-warships', 'common');
	const dog = await cardOf(page, 'dog-breeds', 'common');
	await give(page, ship.slug, 6);
	await give(page, dog.slug, 8);
	await visit(page, '/forge');
	await expect(page.locator('.tray .tcard')).toHaveCount(2);

	await page.getByRole('button', { name: /filter by set/i }).click();
	const picker = page.getByRole('dialog', { name: /choose a set/i });
	await expect(picker.locator('.row')).toHaveCount(3); // all sets + the two sets with cards
	await picker.getByRole('button', { name: /dog breeds/i }).click();
	await expect(page.locator('.tray .tcard')).toHaveCount(1);
	await expect(page.locator('.tray .tcard')).toContainText(dog.title);

	// Fill uses the filtered set only: 7 spare dogs, no ships
	await page.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await expect(page.locator('.core-in b')).toHaveText('7/10');
	await expect(page.locator('.odds li')).toHaveCount(1);
});

// AC-51: single copies did not show in the trade-up tray
test('the trade-up tray lists single copies, and fill leaves them alone', async ({ page }) => {
	await newPlayer(page);
	const col = await (await page.request.get('/api/collections/ww2-warships')).json();
	const singles = col.cards.filter((c: { rarity: string }) => c.rarity === 'common').slice(0, 3);
	for (const c of singles) await give(page, c.slug, 1);
	await visit(page, '/forge');
	await expect(page.locator('.tray .tcard')).toHaveCount(3);
	await expect(page.locator('.tray .c-badge.last')).toHaveCount(3);

	await page.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await expect(page.locator('.core-in b')).toHaveText('0/10');
	// By hand they go in
	await page.locator('.tray .tcard').first().click();
	await expect(page.locator('.core-in b')).toHaveText('1/10');
	await expect(page.locator('.tray .tcard').first()).toBeDisabled();
});
