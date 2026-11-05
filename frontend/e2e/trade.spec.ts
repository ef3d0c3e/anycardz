import { expect, test, type Page } from '@playwright/test';
import { cardOf, give, newPlayer, slide, visit } from './helpers';

const coins = (page: Page) =>
	page
		.locator('#wallet .sr')
		.textContent()
		.then((t) => Number(t?.replace(/\D/g, '')));
const binder = async (page: Page) =>
	Object.fromEntries(
		(
			(await (await page.request.get('/api/binder')).json()).cards as {
				card: string;
				count: number;
			}[]
		).map((c) => [c.card, c.count])
	);

test('two players seal, a change breaks the seals, then the swap goes through', async ({
	browser
}) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	await newPlayer(alice);
	const bobName = await newPlayer(bob);
	const common = await cardOf(alice, 'ww2-warships', 'common');
	const legendary = await cardOf(alice, 'ww2-warships', 'legendary');
	await give(alice, common.slug, 1);
	await give(bob, legendary.slug, 1);

	// Alice finds Bob and opens a trade
	await visit(alice, '/trade');
	await alice.getByRole('searchbox').fill(bobName.slice(0, -2));
	await alice.getByRole('button', { name: `Trade with ${bobName}` }).click();
	await expect(alice).toHaveURL(/\/trade\/\d+/);
	await alice.locator('html[data-hydrated]').waitFor({ state: 'attached' });

	// Asks for Bob's legendary, offers her common, seals
	await alice.locator('.drawer .tcard', { hasText: legendary.title }).click();
	await expect(alice.locator('.theirs .tcards .tcard')).toHaveCount(1);
	await alice.getByRole('button', { name: 'Your cards' }).click();
	await alice.locator('.drawer .tcard', { hasText: common.title }).click();
	await expect(alice.locator('.mine .tcards .tcard')).toHaveCount(1);
	await slide(alice, alice.locator('.bar .slide'));
	await expect(alice.getByRole('status')).toContainText('Waiting for');

	// Bob opens it, adds 50 coins: both seals break and Alice is told
	await visit(bob, '/trade');
	await expect(bob.locator('.trade.turn')).toBeVisible();
	await bob.locator('.trade.turn').click();
	await bob.locator('html[data-hydrated]').waitFor({ state: 'attached' });
	await expect(bob.locator('.theirs .seal')).toBeVisible();
	await bob.locator('#coins-mine').fill('50');
	await bob.locator('#coins-mine').blur();
	await expect(bob.locator('.theirs .seal')).toHaveCount(0);
	await expect(alice.getByText(/changed the offer/)).toBeVisible({ timeout: 6000 });
	await expect(alice.locator('#coins-theirs')).toHaveValue('50');
	await expect(alice.locator('.bar .slide')).toBeVisible();

	// Alice agrees to the new offer, Bob seals and swaps
	await slide(alice, alice.locator('.bar .slide'));
	await expect(bob.getByText('Slide to seal and swap')).toBeVisible({ timeout: 6000 });
	await slide(bob, bob.locator('.bar .slide'));
	await expect(bob.locator('.eyebrow', { hasText: 'Trade done' })).toBeVisible();
	await expect(alice.locator('.eyebrow', { hasText: 'Trade done' })).toBeVisible({ timeout: 6000 });

	expect((await binder(alice))[legendary.slug]).toBe(1);
	expect((await binder(alice))[common.slug]).toBeUndefined();
	expect((await binder(bob))[common.slug]).toBe(1);
	// The 50 coins, plus the first_trade achievement for both
	await expect.poll(() => coins(alice)).toBe(1050 + 150);
	await expect.poll(() => coins(bob)).toBe(950 + 150);
});
