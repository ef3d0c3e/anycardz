import { expect, test, type Page } from '@playwright/test';
import { cardOf, fullyVisible, give, hold, newPlayer, visit } from './helpers';

/** The live panel: a column on desktop, a sheet behind the chat button elsewhere */
async function openPanel(page: Page) {
	const button = page.getByRole('button', { name: /open live pulls and chat/i });
	if (await button.isVisible()) await button.click();
	const panel = page.getByRole('complementary', { name: /live pulls and chat/i });
	await expect(panel).toBeVisible();
	return panel;
}

test('a message reaches the other player live, with an unread count on small screens', async ({
	browser
}) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	await newPlayer(alice);
	await newPlayer(bob);
	await visit(alice, '/');
	await visit(bob, '/');
	// Both sockets are up before anyone speaks
	await expect(bob.locator('.offline')).toHaveCount(0);
	await expect(alice.locator('.offline')).toHaveCount(0);

	const words = `hello from alice ${Date.now()}`;
	const panel = await openPanel(alice);
	await panel.getByRole('button', { name: 'Shoutbox' }).click();
	await panel.getByRole('textbox').fill(words);
	await panel.getByRole('button', { name: 'Send' }).click();
	await expect(panel.getByText(words)).toBeVisible();

	const chatButton = bob.getByRole('button', { name: /open live pulls and chat/i });
	if (await chatButton.isVisible()) await expect(chatButton.locator('.badge')).toBeVisible();
	const bobPanel = await openPanel(bob);
	await bobPanel.getByRole('button', { name: /shoutbox/i }).click();
	await expect(bobPanel.getByText(words)).toBeVisible();
});

test('bragging a card shows it to everyone, and forging shows in live pulls', async ({
	browser
}) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	const aliceName = await newPlayer(alice);
	await newPlayer(bob);
	const legendary = await cardOf(alice, 'ww2-warships', 'legendary');
	const common = await cardOf(alice, 'ww2-warships', 'common');
	await give(alice, legendary.slug, 1);
	await give(alice, common.slug, 11);
	await visit(bob, '/');
	await expect(bob.locator('.offline')).toHaveCount(0);

	// Brag from the binder inspector
	await visit(alice, '/binder?set=ww2-warships');
	await alice.getByRole('button', { name: 'Owned' }).click();
	await alice.locator('.pocket', { hasText: legendary.title }).click();
	await alice.getByRole('button', { name: `Show off ${legendary.title}` }).click();
	await expect(alice.getByText('Posted to the shoutbox')).toBeVisible();

	const panel = await openPanel(bob);
	await panel.getByRole('button', { name: /shoutbox/i }).click();
	// The shoutbox is shared by parallel runs: look at this player's message only
	await expect(
		panel.locator('.msg', { hasText: aliceName }).locator('.brag', { hasText: legendary.title })
	).toBeVisible();

	// A trade-up result goes to everyone's live pulls
	await visit(alice, '/forge');
	await alice.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await hold(alice, alice.locator('.fuse'), 1800);
	await expect(alice.getByRole('dialog').filter({ hasText: 'Forged' })).toBeVisible({
		timeout: 5000
	});
	await panel.getByRole('button', { name: 'Live pulls' }).click();
	await expect(panel.locator('.pull', { hasText: `${aliceName} forged` })).toBeVisible();
});

// Thumbnails of live pulls once sat behind the row background
test('live pull thumbnails show their art', async ({ page }) => {
	await newPlayer(page);
	const common = await cardOf(page, 'ww2-warships', 'common');
	await give(page, common.slug, 11);
	await visit(page, '/forge');
	await page.getByRole('button', { name: /fill from biggest stacks/i }).click();
	await hold(page, page.locator('.fuse'), 1800);
	await page
		.getByRole('dialog')
		.filter({ hasText: 'Forged' })
		.getByRole('button', { name: 'Continue' })
		.click();
	const panel = await openPanel(page);
	const img = panel.locator('.pull', { hasText: 'You forged' }).locator('.thumb img');
	await expect(img).toBeVisible();
	await expect
		.poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth > 0))
		.toBe(true);
	await img.scrollIntoViewIfNeeded();
	expect(await fullyVisible(img)).toBe(true);
});
