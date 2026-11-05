import { expect, test } from '@playwright/test';
import { cardOf, give, newPlayer, sql, visit } from './helpers';

test('scratch tickets come first on the open page, then the packs', async ({ page }) => {
	const name = await newPlayer(page, 1);
	await sql('UPDATE users SET tickets = 2 WHERE username = $1', [name]);
	await visit(page, '/open');
	// The tab counts packs and tickets
	await expect(page.locator('[data-nav="open"] .badge')).toHaveText('3');
	const ticket = page.getByRole('region', { name: 'Scratch ticket' });
	await expect(ticket).toContainText('2 tickets in hand');

	await ticket.getByRole('button', { name: 'Reveal all' }).click();
	await expect(ticket.getByRole('status')).toBeVisible();
	await expect(ticket.locator('.symbol')).toHaveCount(3);
	await expect(page.locator('[data-nav="open"] .badge')).not.toHaveText('3');
	await ticket.getByRole('button', { name: 'Next ticket · 1 left' }).click();

	// A fresh ticket; "Later" goes to the packs
	await expect(ticket.locator('.symbol')).toHaveCount(0);
	await ticket.getByRole('button', { name: /Later: open your pack first/ }).click();
	await expect(page.locator('.tear-zone')).toBeVisible();
	await expect(page.getByRole('heading', { level: 1 })).not.toHaveText('Scratch ticket');
});

test('three sparkles double the foil roulette for a while', async ({ page }) => {
	const name = await newPlayer(page);
	await sql(
		"UPDATE users SET roulette_boost_until = now() + interval '1 hour' WHERE username = $1",
		[name]
	);
	const legendary = await cardOf(page, 'ww2-warships', 'legendary');
	await give(page, legendary.slug, 1);
	await visit(page, '/forge?tab=roulette');
	// 5% for a legendary, doubled
	await expect(page.locator('.math')).toContainText('10%');
	await expect(page.locator('.math .boost')).toContainText('Boost ×2 until');
});

test('scratching by hand draws the ticket at the first touch', async ({ page }) => {
	const name = await newPlayer(page);
	await sql('UPDATE users SET tickets = 1 WHERE username = $1', [name]);
	await visit(page, '/open');
	const ticket = page.getByRole('region', { name: 'Scratch ticket' });
	const played = page.waitForResponse((r) => r.url().endsWith('/api/tickets/scratch'));
	for (const canvas of await ticket.locator('canvas').all()) {
		const b = (await canvas.boundingBox())!;
		await page.mouse.move(b.x + 4, b.y + 6);
		await page.mouse.down();
		for (let y = 6; y < b.height; y += 12)
			for (const x of [4, b.width - 4]) await page.mouse.move(b.x + x, b.y + y, { steps: 3 });
		await page.mouse.up();
	}
	expect((await played).status()).toBe(200);
	await expect(ticket.getByRole('status')).toBeVisible();
	await expect(ticket.locator('.symbol')).toHaveCount(3);
});
