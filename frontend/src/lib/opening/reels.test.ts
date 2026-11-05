import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	landPause,
	playPack,
	ReelRun,
	spinDuration,
	type Mode,
	type ReelCard,
	type ReelStage
} from './reels.ts';

const common: ReelCard = { rarity: 'common', foil: null };
const pack = (...cards: ReelCard[]) =>
	cards.length ? cards : [common, common, common, common, common];

/** A stage whose reels land after their duration, or as soon as the run says stop */
function fakeStage() {
	const spins: { slot: number; ms: number; took: number }[] = [];
	const log: string[] = [];
	const stage: ReelStage = {
		show: async (idx) => void log.push(`show ${idx.join(',')}`),
		spin: (slot, ms, run) =>
			new Promise((resolve) => {
				const t0 = Date.now();
				const tick = () => {
					if (run.stopping || Date.now() - t0 >= ms) {
						spins.push({ slot, ms, took: Date.now() - t0 });
						resolve();
					} else setTimeout(tick, 16);
				};
				tick();
			}),
		land: (slot, i) => void log.push(`land ${i}`),
		tray: (i) => void log.push(`tray ${i}`),
		status: (phase, i) => void log.push(`${phase} ${i}`)
	};
	return { stage, spins, log };
}
const skipped = (s: { ms: number; took: number }) => s.took < s.ms;

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('one by one', () => {
	it('a tap stops the turning reel only', async () => {
		const { stage, spins } = fakeStage();
		const run = new ReelRun();
		const done = playPack(pack(), 'one', run, stage);
		await vi.advanceTimersByTimeAsync(300);
		run.tap();
		await vi.runAllTimersAsync();
		await done;
		expect(spins.map(skipped)).toEqual([true, false, false, false, false]);
	});

	it('a tap on a landed card moves on, without stopping the next reel', async () => {
		const { stage, spins, log } = fakeStage();
		const run = new ReelRun();
		const done = playPack(pack(), 'one', run, stage);
		await vi.advanceTimersByTimeAsync(spinDuration('one', 0, 'common') + 100);
		expect(log.at(-1)).toBe('landed 0');
		run.tap();
		await vi.advanceTimersByTimeAsync(0);
		expect(log.at(-1)).toBe('spinning 1');
		await vi.runAllTimersAsync();
		await done;
		expect(spins.some(skipped)).toBe(false);
	});

	it('a landed card waits for its pause', async () => {
		const { stage, log } = fakeStage();
		const done = playPack(pack({ rarity: 'rare', foil: null }), 'one', new ReelRun(), stage);
		await vi.advanceTimersByTimeAsync(spinDuration('one', 0, 'rare') + 50);
		await vi.advanceTimersByTimeAsync(landPause({ rarity: 'rare', foil: null }) - 200);
		expect(log).not.toContain('tray 0');
		await vi.runAllTimersAsync();
		await done;
		expect(log).toContain('tray 0');
	});
});

describe('all at once', () => {
	it('a tap stops every reel of the pack', async () => {
		const { stage, spins } = fakeStage();
		const run = new ReelRun();
		const done = playPack(pack(), 'all', run, stage);
		await vi.advanceTimersByTimeAsync(300);
		run.tap();
		await vi.runAllTimersAsync();
		await done;
		expect(spins).toHaveLength(5);
		expect(spins.every(skipped)).toBe(true);
	});

	it('reels land left to right, the legendary one last and slower', () => {
		const d = [0, 1, 2, 3].map((i) => spinDuration('all', i, 'common'));
		expect(d).toEqual([...d].sort((a, b) => a - b));
		expect(spinDuration('all', 4, 'legendary')).toBeGreaterThan(spinDuration('all', 4, 'common'));
	});
});

// AC-43: a tap or skip on one pack stopped the reels of every later pack
describe.each<Mode>(['one', 'all'])('%s: nothing carries over to the next pack', (mode) => {
	it.each(['tap', 'skip'] as const)('after a %s', async (action) => {
		const first = fakeStage();
		const run1 = new ReelRun();
		const p1 = playPack(pack(), mode, run1, first.stage);
		await vi.advanceTimersByTimeAsync(300);
		run1[action]();
		await vi.runAllTimersAsync();
		await p1;

		const second = fakeStage();
		const p2 = playPack(pack(), mode, new ReelRun(), second.stage);
		await vi.runAllTimersAsync();
		await p2;
		expect(second.spins).toHaveLength(5);
		expect(second.spins.some(skipped)).toBe(false);
	});
});

describe('skip', () => {
	it('lands every remaining reel and skips the pauses', async () => {
		const { stage, spins, log } = fakeStage();
		const run = new ReelRun();
		const done = playPack(pack(), 'one', run, stage);
		await vi.advanceTimersByTimeAsync(300);
		run.skip();
		await vi.advanceTimersByTimeAsync(50 * 5);
		await done;
		expect(spins.every(skipped)).toBe(true);
		expect(log.filter((l) => l.startsWith('tray'))).toHaveLength(5);
	});
});
