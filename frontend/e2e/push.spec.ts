import { expect, test } from '@playwright/test';
import { newPlayer, visit } from './helpers';

test('settings say when the server has no push key', async ({ page }) => {
	await newPlayer(page);
	await visit(page, '/settings');
	const push = page.getByRole('region', { name: 'Notifications on this device' });
	// The dev and CI servers run without a VAPID key
	await expect(push).toContainText(/not set up on this server|cannot receive/);
});

test('turning push on subscribes the browser and tells the server', async ({ page, context }) => {
	test.skip(
		test.info().project.name !== 'desktop',
		'the stubbed Push API is the same on every device'
	);
	await context.grantPermissions(['notifications']);
	// Headless browsers reach no push service: the Push API answers with a made up subscription
	await page.addInitScript(() => {
		// Some headless builds report notifications as denied whatever was granted
		Object.defineProperty(Notification, 'permission', { get: () => 'granted' });
		Notification.requestPermission = async () => 'granted';
		let sub: object | null = null;
		const fake = {
			endpoint: 'https://fcm.googleapis.com/fcm/send/e2e',
			options: { applicationServerKey: null },
			toJSON: () => ({ endpoint: fake.endpoint, keys: { p256dh: 'BPk', auth: 'YXV0aA' } }),
			unsubscribe: async () => ((sub = null), true)
		};
		PushManager.prototype.subscribe = async function () {
			return (sub = fake) as unknown as PushSubscription;
		};
		PushManager.prototype.getSubscription = async function () {
			return sub as PushSubscription | null;
		};
	});
	let subscribed = false;
	let sent: { endpoint?: string } = {};
	await page.route('**/api/push', async (route) => {
		const method = route.request().method();
		if (method === 'GET') return route.fulfill({ json: { key: 'B' + 'A'.repeat(86), subscribed } });
		if (method === 'PUT') {
			sent = route.request().postDataJSON();
			subscribed = true;
		}
		if (method === 'DELETE') subscribed = false;
		return route.fulfill({ status: 204 });
	});
	await newPlayer(page);
	await visit(page, '/settings');
	const push = page.getByRole('region', { name: 'Notifications on this device' });
	await expect(push).toContainText('Off for this device.');
	await push.getByRole('button', { name: 'Turn on' }).click();
	await expect(push).toContainText('On for this device.');
	expect(sent.endpoint).toBe('https://fcm.googleapis.com/fcm/send/e2e');
	await push.getByRole('button', { name: 'Turn off' }).click();
	await expect(push).toContainText('Off for this device.');
	expect(subscribed).toBe(false);
});
