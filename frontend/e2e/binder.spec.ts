import { expect, test } from '@playwright/test';
import { newPlayer, visit } from './helpers';

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
