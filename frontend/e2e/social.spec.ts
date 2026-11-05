import { expect, test } from '@playwright/test';
import { newPlayer, visit } from './helpers';

// A 1×1 red PNG: the server crops and resizes it to 256 px
const PNG = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
	'base64'
);

test('a player edits their bio and avatar', async ({ page }) => {
	const name = await newPlayer(page);
	await visit(page, '/me');
	await page.locator('#bio').fill('Collector of warships');
	await page.getByRole('button', { name: 'Save' }).click();
	await expect(page.getByText('Profile saved')).toBeVisible();

	await page
		.locator('#avatar-file')
		.setInputFiles({ name: 'me.png', mimeType: 'image/png', buffer: PNG });
	await expect(page.getByText('Avatar updated')).toBeVisible();
	// The top bar shows it at once
	await expect(page.locator('.top .avatar img')).toHaveAttribute(
		'src',
		new RegExp(`/api/avatars/${name}\\?v=`)
	);

	await page.getByRole('link', { name: 'See your public profile' }).click();
	await expect(page.locator('h1')).toHaveText(name);
	await expect(page.getByText('Collector of warships')).toBeVisible();
	await expect(page.locator('.head .avatar img')).toBeVisible();
});

test('a friend request, accepted from the friends page', async ({ browser }) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	const aliceName = await newPlayer(alice);
	const bobName = await newPlayer(bob);

	await visit(alice, `/players/${bobName}`);
	await alice.getByRole('button', { name: 'Add friend' }).click();
	await expect(alice.getByText('Request sent')).toBeVisible();

	await visit(bob, '/');
	// AC-30: the bell counts it
	await expect(bob.locator('.bell .badge')).toHaveText('1');
	await bob.locator('.bell').click();
	await expect(bob.getByRole('dialog', { name: 'Notifications' })).toContainText(
		`${aliceName} wants to be your friend`
	);
	await bob.keyboard.press('Escape');
	await visit(bob, '/friends');
	const request = bob.locator('.row.incoming', { hasText: aliceName });
	await request.getByRole('button', { name: 'Accept request' }).click();
	await expect(bob.locator('.row.incoming')).toHaveCount(0);
	await expect(bob.locator('.row', { hasText: aliceName })).toBeVisible();
	await expect(bob.locator('.bell .badge')).toHaveCount(0);

	await alice.reload();
	await expect(alice.getByText('✓ Friends')).toBeVisible();
});

test('blocking hides messages and refuses trades', async ({ browser }) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	await newPlayer(alice);
	const bobName = await newPlayer(bob);
	const words = `bob says hi ${Date.now()}`;
	expect((await bob.request.post('/api/shoutbox', { data: { body: words } })).status()).toBe(201);

	await visit(alice, `/players/${bobName}`);
	await alice.getByRole('button', { name: 'Block' }).click();
	await alice.getByRole('alertdialog').getByRole('button', { name: 'Block' }).click();
	await expect(alice.getByText(`You blocked ${bobName}`)).toBeVisible();
	await expect(alice.getByRole('button', { name: `Trade with ${bobName}` })).toHaveCount(0);

	// The shoutbox, after a fresh load, leaves Bob out
	await visit(alice, '/');
	const button = alice.getByRole('button', { name: /open live pulls and chat/i });
	if (await button.isVisible()) await button.click();
	const panel = alice.getByRole('complementary', { name: /live pulls and chat/i });
	await panel.getByRole('button', { name: /shoutbox/i }).click();
	await expect(panel.locator('.msg').first()).toBeVisible();
	await expect(panel.getByText(words)).toHaveCount(0);
	expect((await alice.request.post('/api/trades', { data: { with: bobName } })).status()).toBe(409);
});

// AC-30: notifications arrive live and link to what they are about
test('a trade opened by a friend notifies live, and the bell links to it', async ({ browser }) => {
	const alice = await (await browser.newContext()).newPage();
	const bob = await (await browser.newContext()).newPage();
	const aliceName = await newPlayer(alice);
	const bobName = await newPlayer(bob);
	await visit(bob, '/');
	await expect(bob.locator('.offline')).toHaveCount(0);

	const res = await alice.request.post('/api/trades', { data: { with: bobName } });
	const { id } = await res.json();
	await expect(bob.getByText(`${aliceName} opened a trade with you`)).toBeVisible();
	await expect(bob.locator('.bell .badge')).toHaveText('1');

	await bob.locator('.bell').click();
	await bob
		.getByRole('dialog', { name: 'Notifications' })
		.getByRole('link', { name: /opened a trade/ })
		.click();
	await expect(bob).toHaveURL(new RegExp(`/trade/${id}$`));
	await expect(bob.locator('.bell .badge')).toHaveCount(0);
});
