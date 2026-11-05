import { expect, test } from '@playwright/test';
import { give, newPlayer, visit } from './helpers';

test('the set picker searches and switches sets', async ({ page }) => {
	await newPlayer(page);
	await visit(page, '/binder');
	await page.locator('.set-btn').click();
	const picker = page.getByRole('dialog', { name: /choose a set/i });
	await expect(picker).toBeVisible();

	const total = await picker.locator('.row').count();
	expect(total).toBeGreaterThan(1);
	await picker.getByRole('searchbox').fill('dog');
	await expect(picker.locator('.row')).toHaveCount(1);
	await page.keyboard.press('Enter');
	await expect(picker).toBeHidden();
	await expect(page.locator('.set-title')).toContainText('Dog');
	await expect(page).toHaveURL(/set=dog-breeds/);
});

test('/ opens the set picker, the arrows step through sets', async ({ page }) => {
	await newPlayer(page);
	await visit(page, '/binder');
	const first = await page.locator('.set-title').textContent();
	await page.getByRole('button', { name: /next set/i }).click();
	await expect(page.locator('.set-title')).not.toHaveText(first!);
	await page.getByRole('button', { name: /previous set/i }).click();
	await expect(page.locator('.set-title')).toHaveText(first!);

	await page.keyboard.press('/');
	await expect(page.getByRole('dialog', { name: /choose a set/i })).toBeVisible();
});

test('the binder opens on the set closest to complete', async ({ page }) => {
	await newPlayer(page);
	const sets: { slug: string; title: string }[] = await (
		await page.request.get('/api/collections')
	).json();
	const cards = async (slug: string) =>
		(
			(await (await page.request.get(`/api/collections/${slug}`)).json()).cards as {
				slug: string;
			}[]
		).map((c) => c.slug);
	// A few cards of the first set, most of a smaller one
	const [big, small] = [sets[0], sets.find((s) => s.slug === 'desserts')!];
	for (const slug of (await cards(big.slug)).slice(0, 3)) await give(page, slug, 1);
	const desserts = await cards(small.slug);
	for (const slug of desserts.slice(0, desserts.length - 2)) await give(page, slug, 1);
	await visit(page, '/binder');
	await expect(page.locator('.set-title')).toContainText(small.title);
});

test('on a phone the set picker does not raise the keyboard', async ({ page, isMobile }) => {
	test.skip(!isMobile, 'touch screens only');
	await newPlayer(page);
	await visit(page, '/binder');
	await page.locator('.set-btn').click();
	await expect(page.getByRole('dialog', { name: /choose a set/i })).toBeVisible();
	await expect(page.getByRole('searchbox')).not.toBeFocused();
});
