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

const overlaps = (a: { x: number; y: number; width: number; height: number }, b: typeof a) =>
	a.x < b.x + b.width - 1 &&
	b.x < a.x + a.width - 1 &&
	a.y < b.y + b.height - 1 &&
	b.y < a.y + a.height - 1;

// AC-52: more than 3 packs in the pile covered the text beside them
test('a big pile stays clear of its text, even fanned out', async ({ page }) => {
	await newPlayer(page, 4); // the reported case: 4 packs
	await visit(page, '/');
	const text = page.locator('.pile-text');
	const hero = (await page.locator('.pile-hero').boundingBox())!;
	for (const hover of [false, true]) {
		if (hover) await page.locator('.pile-stack').hover();
		await page.waitForTimeout(450);
		const t = (await text.boundingBox())!;
		for (const pack of await page.locator('.pile-stack .pack').all()) {
			// Rotated packs: their transformed box is what the eye sees
			const r = await pack.evaluate((el) => el.getBoundingClientRect().toJSON());
			expect(overlaps(r, t), `pack over the text (hover: ${hover})`).toBe(false);
		}
		// Nothing in the text column spills out of the panel
		for (const el of await text.locator('h2, p, .btn').all()) {
			const b = (await el.boundingBox())!;
			expect(b.x + b.width).toBeLessThanOrEqual(hero.x + hero.width + 1);
		}
	}
});

test('a pile above three packs shows a count', async ({ page }) => {
	await newPlayer(page, 4);
	await page.request.post('/api/shop/topup', { data: { pack: 'medium' } });
	await page.request.post('/api/shop/boosters', { data: { collection: 'desserts' } });
	await visit(page, '/');
	await expect(page.locator('.pile-stack .pack')).toHaveCount(3);
	await expect(page.locator('.pile-stack .more')).toHaveText('+2');
});

// AC-53: "Recharger pour acheter" pushed the price out of the button in French
for (const lang of ['en', 'fr']) {
	test(`buy buttons and pack labels fit in ${lang}`, async ({ page, context }) => {
		await context.addCookies([{ name: 'lang', value: lang, url: 'http://localhost:5173' }]);
		await newPlayer(page, 4); // 0 coins: every button in its longer "top up" form
		await visit(page, '/');
		for (const button of await page.locator('.shelf .buy').all()) {
			const fits = await button.evaluate((b) => b.scrollWidth <= b.clientWidth + 1);
			expect(fits, await button.getAttribute('aria-label')).toBe(true);
			const price = (await button.locator('.price').boundingBox())!;
			const box = (await button.boundingBox())!;
			expect(price.x).toBeGreaterThanOrEqual(box.x);
			expect(price.x + price.width).toBeLessThanOrEqual(box.x + box.width);
		}
		await expect(page.locator('.shelf .buy').first()).toHaveAccessibleName(
			lang === 'fr' ? /recharger pour acheter un booster/i : /top up to buy a .* booster/i
		);
		// The "5 cards" label never runs into the featured card
		for (const pack of await page.locator('.shelf .pk').all()) {
			const meta = (await pack.locator('.pk-meta').boundingBox())!;
			const card = await pack
				.locator('.pk-card')
				.evaluate((el) => el.getBoundingClientRect().toJSON());
			expect(overlaps(meta, card)).toBe(false);
		}
	});
}

// AC-42: the shelf grew with every collection
test('the shelf stays short, the rest is one search away', async ({ page }) => {
	await newPlayer(page);
	const sets = (await (await page.request.get('/api/collections')).json()) as {
		slug: string;
		title: string;
	}[];
	await visit(page, '/');
	const browse = page.getByRole('button', { name: /browse all \d+ sets/i });
	if (sets.length <= 10) {
		await expect(page.locator('.shelf-item')).toHaveCount(sets.length);
		await expect(browse).toHaveCount(0);
		return;
	}
	await expect(page.locator('.shelf-item')).toHaveCount(10);
	// A set that is not on the shelf, picked from the full list, comes first
	const onShelf = await page.locator('.shelf-item h3').allTextContents();
	const missing = sets.find((s) => !onShelf.includes(s.title))!;
	await browse.click();
	const picker = page.getByRole('dialog', { name: /choose a set/i });
	await picker.getByRole('searchbox').fill(missing.title);
	await page.keyboard.press('Enter');
	await expect(picker).toBeHidden();
	await expect(page.locator('.shelf-item h3').first()).toHaveText(missing.title);
	await expect(page.locator('.shelf-item')).toHaveCount(10);
	// Remembered on the next visit
	await page.reload();
	await expect(page.locator('.shelf-item h3').first()).toHaveText(missing.title);
});
