import { expect, test, type Page } from '@playwright/test';
import { cardOf, give, hold, newPlayer, visit } from './helpers';

const coins = (page: Page) =>
	page
		.locator('#wallet .sr')
		.textContent()
		.then((t) => Number(t?.replace(/\D/g, '')));

/** Lists a copy from the binder inspector */
async function listFromBinder(page: Page, title: string, price: number) {
	await visit(page, '/binder?set=ww2-warships');
	await page.getByRole('button', { name: 'Owned' }).click();
	await page.locator('.pocket', { hasText: title }).click();
	const sheet = page.getByRole('dialog', { name: title });
	await sheet.getByRole('button', { name: 'Sell on the market' }).click();
	await sheet.getByRole('spinbutton', { name: 'Price' }).fill(String(price));
	await sheet.getByRole('button', { name: `List for ${price} coins` }).click();
	await expect(page.getByText(`${title} listed for ${price} coins`)).toBeVisible();
	return sheet;
}

test('a card listed from the binder is bought on the market', async ({ browser }) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	const aliceName = await newPlayer(alice);
	const bobName = await newPlayer(bob);
	const rare = await cardOf(alice, 'ww2-warships', 'rare');
	await give(alice, rare.slug, 2);

	const sheet = await listFromBinder(alice, rare.title, 120);
	// The listed copy left the binder
	await expect(sheet.getByText('1 copy')).toBeVisible();
	await sheet.getByRole('button', { name: 'Close' }).click();

	await visit(bob, '/market');
	await expect(bob.locator('[data-nav="trade"]')).toHaveAttribute('aria-current', 'page');
	await bob.getByRole('searchbox', { name: 'Search cards on sale' }).fill(rare.title);
	await expect(bob).toHaveURL(/q=/);
	const offer = bob.locator('.listing', { hasText: aliceName });
	await hold(
		bob,
		offer.getByRole('button', { name: `Hold to buy ${rare.title} for 120 coins` }),
		900
	);
	await expect(bob.getByText(`${rare.title} is in your binder`)).toBeVisible();
	await expect(offer).toHaveCount(0);
	expect(await coins(bob)).toBe(1000 - 120);

	// The seller hears of it, gets 114 (120 less the 6 coin fee) and sees the sale
	await expect(
		alice.getByText(`${bobName} bought one of your listings · +114 coins`)
	).toBeVisible();
	await visit(alice, '/market');
	await expect(alice.locator('#selling')).toContainText(`Sold to ${bobName}`);
});

test('a listing can be withdrawn, and the copy comes back', async ({ page }) => {
	await newPlayer(page);
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, common.slug, 1);
	await listFromBinder(page, common.title, 30);
	await visit(page, '/market');
	const mine = page.locator('#selling');
	await expect(mine).toContainText('1 of 20 listed');
	await mine.getByRole('button', { name: `Withdraw ${common.title} from the market` }).click();
	await expect(page.getByText(`${common.title} is back in your binder`)).toBeVisible();
	await expect(mine).toContainText('0 of 20 listed');
	const binder = await (await page.request.get('/api/binder')).json();
	expect(binder.cards.find((c: { card: string }) => c.card === common.slug)?.count).toBe(1);
});
