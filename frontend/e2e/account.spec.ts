import { expect, test } from '@playwright/test';
import { newPlayer, sql, visit } from './helpers';

test('sign-up needs the terms ticked', async ({ page }) => {
	const name = `pw${Date.now().toString(36)}${Math.floor(Math.random() * 1e4)}`;
	await visit(page, '/register');
	await page.locator('#username').fill(name);
	await page.locator('#email').fill(`${name}@example.com`);
	await page.locator('#password').fill('123456789');
	await page.getByRole('button', { name: /create|sign up|créer/i }).click();
	await expect(page.getByRole('alert')).toContainText(/accept the terms/i);
	await expect(page).toHaveURL(/register/);

	// The links open the texts without losing the form
	const [terms] = await Promise.all([
		page.waitForEvent('popup'),
		page.getByRole('link', { name: 'terms of service', exact: true }).click()
	]);
	await expect(terms.getByRole('heading', { level: 1 })).toHaveText('Terms of service');
	await terms.close();

	await page.locator('#terms').check();
	await page.getByRole('button', { name: /create|sign up|créer/i }).click();
	await expect(page).toHaveURL(/\/$/);
	await expect(page.getByRole('alertdialog')).toHaveCount(0);
});

test('new terms are asked once, settings stay reachable', async ({ page }) => {
	const name = await newPlayer(page);
	await sql('UPDATE users SET terms_version = NULL WHERE username = $1', [name]);
	await visit(page, '/binder');
	const gate = page.getByRole('alertdialog', { name: /terms have changed/i });
	await expect(gate).toBeVisible();

	// The way out: settings, where the data can be exported or the account deleted
	await gate.getByRole('link', { name: 'Settings' }).click();
	await expect(page).toHaveURL(/settings/);
	await expect(gate).toHaveCount(0);

	await visit(page, '/');
	await gate.getByRole('button', { name: 'I accept' }).click();
	await expect(gate).toHaveCount(0);
	await visit(page, '/');
	await expect(page.locator('#wallet')).toBeVisible();
	await expect(gate).toHaveCount(0);
});

test('password change signs the other devices out', async ({ page, browser }) => {
	const name = await newPlayer(page);
	const other = await browser.newContext();
	const phone = await other.newPage();
	const res = await phone.request.post('http://localhost:5173/api/auth/login', {
		data: { login: name, password: '123456789' },
		headers: {
			'user-agent': 'Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0'
		}
	});
	expect(res.ok()).toBe(true);

	await visit(page, '/settings');
	const devices = page.locator('.session');
	await expect(devices).toHaveCount(2);
	await expect(devices.filter({ hasText: 'This device' })).toHaveCount(1);
	await expect(devices.filter({ hasText: 'Firefox · Linux' })).toHaveCount(1);

	await page.locator('#current-password').fill('wrong-wrong');
	await page.locator('#new-password').fill('a-new-password');
	await page.getByRole('button', { name: 'Change password' }).click();
	await expect(page.getByRole('alert')).toHaveText('That is not your current password.');

	await page.locator('#current-password').fill('123456789');
	await page.getByRole('button', { name: 'Change password' }).click();
	await expect(page.getByText(/other devices are signed out/)).toBeVisible();
	await expect(devices).toHaveCount(1);
	expect((await phone.request.get('http://localhost:5173/api/me')).status()).toBe(401);
	await other.close();
});

test('data export downloads a file with the account in it', async ({ page }) => {
	const name = await newPlayer(page);
	await visit(page, '/settings');
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('link', { name: 'Download my data' }).click()
	]);
	expect(download.suggestedFilename()).toMatch(new RegExp(`^anycardz-${name}-.*\\.json$`));
	const path = await download.path();
	const data = JSON.parse((await import('node:fs')).readFileSync(path, 'utf8'));
	expect(data.account.username).toBe(name);
});

test('deleting the account takes the username typed again', async ({ page }) => {
	const name = await newPlayer(page);
	await visit(page, '/settings');
	const remove = page.getByRole('button', { name: 'Delete my account forever' });
	await expect(remove).toBeDisabled();
	await page.locator('#delete-confirm').fill(name.toUpperCase());
	await page.locator('#delete-password').fill('wrong-wrong');
	await remove.click();
	await expect(page.getByRole('alert')).toHaveText('That is not your current password.');

	await page.locator('#delete-password').fill('123456789');
	await remove.click();
	await expect(page).toHaveURL(/login/);
	const login = await page.request.post('/api/auth/login', {
		data: { login: name, password: '123456789' }
	});
	expect(login.status()).toBe(422);
});

test('terms and privacy read well on a phone, in both languages', async ({ page }) => {
	for (const [lang, title] of [
		['en', 'Privacy notice'],
		['fr', 'Confidentialité']
	]) {
		await page.context().addCookies([{ name: 'lang', value: lang, url: 'http://localhost:5173' }]);
		await visit(page, '/privacy');
		await expect(page.getByRole('heading', { level: 1 })).toHaveText(title);
		const overflow = await page.evaluate(
			() => document.documentElement.scrollWidth - document.documentElement.clientWidth
		);
		expect(overflow).toBeLessThanOrEqual(0);
	}
});
