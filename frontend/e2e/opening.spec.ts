import { expect, test } from '@playwright/test';
import { newPlayer, reelMode, tearPack, visit } from './helpers';

// AC-43: stopping the reels of one pack used to make every later pack land at once.
// A reel takes at least 1.9 s to land on its own, so a reel landed within 0.8 s was skipped.
const UNSKIPPED = 800;

for (const mode of ['one', 'all'] as const) {
	test.describe(`reels ${mode === 'one' ? 'one by one' : 'all at once'}`, () => {
		test('tapping to stop does not carry over to the next pack', async ({ page }) => {
			await newPlayer(page, 2);
			await reelMode(page, mode);
			await visit(page, '/open');

			await tearPack(page);
			await page.locator('.arena').click();
			await expect(page.locator('.reel.landed').first()).toBeVisible({ timeout: 1500 });
			// All at once, the tap may already have finished the pack: Skip is gone then
			const skip = page.getByRole('button', { name: /skip/i });
			if (await skip.isVisible()) await skip.click({ timeout: 1000 }).catch(() => {});
			await page.locator('.after').waitFor();

			await page.locator('.after .btn.primary').click();
			await tearPack(page);
			await page.waitForTimeout(UNSKIPPED);
			await expect(page.locator('.reel.landed')).toHaveCount(0);
			// It still lands by itself
			await expect(page.locator('.reel.landed').first()).toBeVisible({ timeout: 5000 });
		});
	});
}

test('skip lands the whole pack, not the next one', async ({ page }) => {
	await newPlayer(page, 2);
	await reelMode(page, 'all');
	await visit(page, '/open');

	await tearPack(page);
	await page.getByRole('button', { name: /skip/i }).click();
	await expect(page.locator('.reel.landed')).toHaveCount(5, { timeout: 1500 });
	await page.locator('.after .btn.primary').click();

	await tearPack(page);
	await page.waitForTimeout(UNSKIPPED);
	await expect(page.locator('.reel.landed')).toHaveCount(0);
});

test('a pack lands five cards in the tray', async ({ page }) => {
	await newPlayer(page, 1);
	await reelMode(page, 'all');
	await visit(page, '/open');
	await tearPack(page);
	await page.getByRole('button', { name: /skip/i }).click();
	await expect(page.locator('.tray .slotmini.pop')).toHaveCount(5);
	await expect(page.locator('.after')).toBeVisible();
});

// Achievements would tell what the pack holds: they wait for the reveal
test('achievements wait until the pack is revealed', async ({ page }) => {
	await newPlayer(page, 1);
	await reelMode(page, 'one');
	// When the first achievement toast shows up, and when the pack summary does
	await page.addInitScript(() => {
		const w = window as unknown as { seen: Record<string, number> };
		w.seen = {};
		new MutationObserver(() => {
			if (!w.seen.toast && document.querySelector('.toasts')?.textContent?.includes('First tear'))
				w.seen.toast = performance.now();
			if (!w.seen.after && document.querySelector('.after')) w.seen.after = performance.now();
		}).observe(document, { subtree: true, childList: true, characterData: true });
	});
	await visit(page, '/open');
	await tearPack(page);
	// The pity counter is up to date from the start of the spin
	await expect(page.getByText(/Legendary guaranteed/)).toBeVisible();
	await page.waitForTimeout(1500);
	await page.getByRole('button', { name: /skip/i }).click();
	await page.locator('.after').waitFor();
	await expect(page.getByText(/Achievement: First tear/).first()).toBeVisible({ timeout: 5000 });
	const seen = await page.evaluate(
		() => (window as unknown as { seen: Record<string, number> }).seen
	);
	expect(seen.toast).toBeGreaterThanOrEqual(seen.after);
});
