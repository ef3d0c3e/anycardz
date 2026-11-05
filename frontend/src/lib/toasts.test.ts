import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toasts } from './session.svelte.ts';

describe('toasts', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('merges the same message into one, with a count', () => {
		const t = new Toasts();
		for (let i = 0; i < 5; i++) t.show('Booster added');
		expect(t.list.map((x) => [x.text, x.count])).toEqual([['Booster added', 5]]);
	});

	it('keeps two on screen and drains a burst in seconds', () => {
		const t = new Toasts();
		for (const n of ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h']) t.show(n);
		expect(t.list.map((x) => x.text)).toEqual(['a', 'b']);
		// Eight different messages: gone in well under ten seconds, the oldest dropped
		vi.advanceTimersByTime(8000);
		expect(t.list).toEqual([]);
	});

	it('never drops a warning', () => {
		const t = new Toasts();
		t.show('broke', 'warn');
		for (const n of ['a', 'b', 'c', 'd', 'e', 'f']) t.show(n);
		const seen = new Set<string>();
		for (let i = 0; i < 20; i++) {
			for (const x of t.list) seen.add(x.text);
			vi.advanceTimersByTime(500);
		}
		expect(seen.has('broke')).toBe(true);
	});
});
