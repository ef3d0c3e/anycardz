import { expect, test } from '@playwright/test';
import { newPlayer, reelMode, sql, tearPack, visit } from './helpers';

test('completing a set pays, shows in the binder, on the profile and in the live feed', async ({
	page,
	browser
}) => {
	const name = await newPlayer(page, 1, 'desserts');
	const set = await (await page.request.get('/api/collections/desserts')).json();
	const me = await (await page.request.get('/api/me')).json();
	// The whole set already in the binder: the next card in completes it
	await sql('INSERT INTO owned_cards (user_id, card) SELECT $1, unnest($2::text[])', [
		me.id,
		set.cards.map((c: { slug: string }) => c.slug)
	]);
	await visit(page, '/binder?set=desserts');
	await expect(page.getByText(`Complete the set: +${set.set_reward} coins`)).toBeVisible();
	await expect(page.getByText(/^Foil set 0\/98/)).toBeVisible();

	// Someone else watching the live feed
	// On a wide screen, where the live column is always open
	const other = await browser.newContext({ viewport: { width: 1440, height: 900 } });
	const watcher = await other.newPage();
	await newPlayer(watcher);
	await visit(watcher, '/');

	await reelMode(page, 'all');
	await visit(page, '/open');
	const opened = page.waitForResponse((r) => r.url().endsWith('/api/pile/open'));
	await tearPack(page);
	// The set, and what else this pack earned (first pack, first set, maybe more)
	const unlocked: { key: string; coins: number }[] = (await (await opened).json()).unlocked;
	expect(unlocked.map((u) => u.key)).toContain('set:desserts');
	const paid = unlocked.reduce((n, u) => n + u.coins, 0);
	await expect(page.getByText('Set complete: Sweet Desserts').first()).toBeVisible({
		timeout: 10_000
	});
	await expect
		.poll(async () => (await (await page.request.get('/api/me')).json()).coins)
		.toBe(1000 - 250 + paid);

	await visit(page, '/binder?set=desserts');
	await expect(page.getByText('✓ Set complete')).toBeVisible();
	await visit(page, `/players/${name}`);
	await expect(page.getByRole('link', { name: /Sweet Desserts/ })).toHaveAttribute(
		'href',
		'/binder?set=desserts'
	);

	await expect(
		watcher.getByRole('link', { name: `${name} completed Sweet Desserts` })
	).toBeVisible();
	await other.close();
});
