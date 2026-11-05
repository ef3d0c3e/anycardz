import { expect, test } from '@playwright/test';
import { cardOf, fullyVisible, give, newPlayer, reelMode, tearPack, visit } from './helpers';

// AC-48: tilting cards clipped their own badges, and the reel window clipped the landed card's
test('binder badges stay whole while a card tilts', async ({ page }) => {
	await newPlayer(page);
	const rare = await cardOf(page, 'ww2-warships', 'rare');
	await give(page, rare.slug, 2);
	await give(page, rare.slug, 1, 'foil2');
	await visit(page, '/binder?set=ww2-warships');
	await page.getByRole('button', { name: 'Owned' }).click();
	const pocket = page.locator('.pocket').first();
	const box = (await pocket.boundingBox())!;
	// Pointer near the top right corner: the strongest tilt toward the badge
	await page.mouse.move(box.x + box.width * 0.9, box.y + box.height * 0.1);
	await expect(pocket.locator('.face.tilting')).toHaveCount(1);
	for (const badge of await pocket.locator('.c-badge').all()) {
		expect(await fullyVisible(badge), await badge.textContent()).toBe(true);
	}
});

for (const mode of ['one', 'all'] as const) {
	test(`the landed card's badge is not clipped by the reel (${mode})`, async ({ page }) => {
		await newPlayer(page, 1);
		await reelMode(page, mode);
		await visit(page, '/open');
		await tearPack(page);
		// Tap: the (first) reel lands and stays on screen
		await page.locator('.arena').click();
		const landed = page.locator('.reel.landed').first();
		await expect(landed).toBeVisible();
		// The landing pop (0.45 s) briefly scales the neighbouring cards
		await page.waitForTimeout(500);
		const card = landed.locator('.landed-card .card');
		const box = (await card.boundingBox())!;
		// Toward the top right, below the badge (on tiny cards it covers the corner)
		await page.mouse.move(box.x + box.width * 0.75, box.y + box.height * 0.35);
		await expect(landed.locator('.face.tilting')).toHaveCount(1);
		expect(await fullyVisible(landed.locator('.c-badge').first())).toBe(true);
	});
}

// AC-47: the rarity tell framed the whole reel slot instead of the card
test('the rarity frame is the size of the card on the payline', async ({ page }) => {
	await newPlayer(page, 1);
	await reelMode(page, 'one');
	await visit(page, '/open');
	await tearPack(page);
	const frame = (await page.locator('.payline').first().boundingBox())!;
	const card = (await page.locator('.cell.win .card').first().boundingBox())!;
	const reel = (await page.locator('.reel').first().boundingBox())!;
	expect(Math.abs(frame.width - card.width)).toBeLessThan(2);
	expect(Math.abs(frame.height - card.height)).toBeLessThan(2);
	expect(frame.height).toBeLessThan(reel.height - 20);
});

// AC-49: the tell drew a ring on top of the card's own rarity border
test('the rarity tell glows without a second border', async ({ page }) => {
	await newPlayer(page, 1);
	await reelMode(page, 'one');
	await visit(page, '/open');
	await tearPack(page);
	for (const tell of ['tell-rare', 'tell-legendary']) {
		const shadow = await page
			.locator('.reel')
			.first()
			.evaluate(async (reel, cls) => {
				reel.classList.add(cls);
				// The glow fades in over 0.4 s
				await new Promise((r) => setTimeout(r, 500));
				const s = getComputedStyle(reel.querySelector('.payline')!).boxShadow;
				reel.classList.remove(cls);
				return s;
			}, tell);
		expect(shadow, tell).not.toBe('none');
		// A ring is a shadow with no blur and a spread: "0px 0px 0px 2px"
		expect(shadow, tell).not.toMatch(/0px 0px 0px \d/);
	}
});

// The bell and search pushed the account button off narrow phone screens
for (const width of [360, 390]) {
	test(`every top bar control fits at ${width} px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 800 });
		await newPlayer(page);
		await visit(page, '/');
		const controls = page.locator('.top').getByRole('button');
		expect(await controls.count()).toBeGreaterThanOrEqual(5);
		for (const control of await controls.all()) {
			if (!(await control.isVisible())) continue;
			const box = (await control.boundingBox())!;
			expect(box.x, (await control.getAttribute('aria-label')) ?? '').toBeGreaterThanOrEqual(0);
			expect(
				box.x + box.width,
				(await control.getAttribute('aria-label')) ?? ''
			).toBeLessThanOrEqual(width);
		}
		expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
			width
		);
	});
}
