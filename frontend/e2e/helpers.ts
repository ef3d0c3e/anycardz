import { expect, type Page } from '@playwright/test';

/** A fresh player, logged in through the page's own cookie jar */
export async function newPlayer(page: Page, boosters = 0, collection = 'ww2-warships') {
	const name = `pw${Date.now().toString(36)}${Math.floor(Math.random() * 1e4)}`;
	const res = await page.request.post('/api/auth/register', {
		data: { username: name, email: `${name}@example.com`, password: '123456789', terms: 1 }
	});
	expect(res.status()).toBe(201);
	// Random daily missions pay coins: a placeholder keeps them out of tests that count
	await setMissions(name, ['none']);
	// Scratch tickets would come first on the Open page: tests that want them set them
	await sql('UPDATE users SET tickets = 0 WHERE username = $1', [name]);
	for (let i = 0; i < boosters; i++) {
		expect((await page.request.post('/api/shop/boosters', { data: { collection } })).ok()).toBe(
			true
		);
	}
	return name;
}

/** Sets a player's missions for today (keys from the API's list) */
export async function setMissions(name: string, keys: string[]) {
	await sql(
		'DELETE FROM daily_missions WHERE user_id = (SELECT id FROM users WHERE username = $1)',
		[name]
	);
	await sql(
		`INSERT INTO daily_missions (user_id, day, key)
		 SELECT (SELECT id FROM users WHERE username = $1), (now() AT TIME ZONE 'utc')::date, k
		 FROM unnest($2::text[]) k`,
		[name, keys]
	);
}

/** Picks the reel mode before the app loads, as the mode switch would */
export async function reelMode(page: Page, mode: 'one' | 'all') {
	await page.addInitScript((m) => localStorage.setItem('anycardz-reel-mode', m), mode);
}

/** Tears the pack open with the keyboard and waits for the reels to appear */
export async function tearPack(page: Page) {
	await page.locator('.tear-zone').waitFor();
	await page.locator('.arena').press('Space');
	await page.locator('.reel').first().waitFor();
}

/** Navigates and waits for hydration: before it, clicks on SSR markup do nothing */
export async function visit(page: Page, path: string) {
	await page.goto(path);
	await page.locator('html[data-hydrated]').waitFor({ state: 'attached' });
}

/** Runs SQL on the dev database (see .env), for states the UI cannot reach quickly */
export async function sql(query: string, params: unknown[] = []) {
	const pg = await import('pg');
	const client = new pg.default.Client({
		connectionString:
			process.env.DATABASE_URL ?? 'postgres://anycardz:anycardz@127.0.0.1:5433/anycardz'
	});
	await client.connect();
	try {
		return await client.query(query, params);
	} finally {
		await client.end();
	}
}

/** Inserts copies straight into the player's binder */
export async function give(page: Page, card: string, count: number, foil: string | null = null) {
	const me = await (await page.request.get('/api/me')).json();
	await sql(
		'INSERT INTO owned_cards (user_id, card, foil) SELECT $1, $2, $3 FROM generate_series(1, $4)',
		[me.id, card, foil, count]
	);
}

/** First card of a rarity in a collection, from the real catalog */
export async function cardOf(
	page: Page,
	collection: string,
	rarity: 'common' | 'rare' | 'legendary'
) {
	const col = await (await page.request.get(`/api/collections/${collection}`)).json();
	return col.cards.find((c: { rarity: string }) => c.rarity === rarity) as {
		slug: string;
		title: string;
	};
}

/** Presses and holds a hold-to-confirm button */
export async function hold(page: Page, locator: import('@playwright/test').Locator, ms: number) {
	const box = (await locator.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(ms);
	await page.mouse.up();
}

/** Drags a slide-to-confirm knob to the end */
export async function slide(page: Page, root: import('@playwright/test').Locator) {
	const track = (await root.boundingBox())!;
	const knob = (await root.locator('.knob').boundingBox())!;
	const y = knob.y + knob.height / 2;
	await page.mouse.move(knob.x + knob.width / 2, y);
	await page.mouse.down();
	for (let i = 1; i <= 10; i++)
		await page.mouse.move(knob.x + ((track.x + track.width - knob.x) * i) / 10, y);
	await page.mouse.up();
}

/**
 * True when the element's edges are painted on top: a clipped or covered
 * badge fails (clipped areas are not hit-tested).
 */
export async function fullyVisible(locator: import('@playwright/test').Locator) {
	return locator.evaluate((el) => {
		const r = el.getBoundingClientRect();
		// Edge midpoints: corners of a rounded badge are outside its shape
		const inset = 3;
		const cx = (r.left + r.right) / 2;
		const cy = (r.top + r.bottom) / 2;
		const points = [
			[r.left + inset, cy],
			[r.right - inset, cy],
			[cx, r.top + inset],
			[cx, r.bottom - inset]
		];
		const misses = points
			.map(([x, y]) => ({ x, y, hit: document.elementFromPoint(x, y) }))
			.filter(({ hit }) => !hit || !(hit === el || el.contains(hit)))
			.map(
				({ x, y, hit }) =>
					`(${Math.round(x)},${Math.round(y)}) ${hit?.tagName}.${hit?.getAttribute('class') ?? ''}`
			);
		return misses.length ? misses.join('; ') : true;
	});
}
