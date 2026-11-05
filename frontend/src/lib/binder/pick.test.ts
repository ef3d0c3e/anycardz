import { describe, expect, it } from 'vitest';
import { defaultSet } from './pick.ts';

const sets = [
	{ slug: 'gems', total: 10 },
	{ slug: 'ships', total: 150 },
	{ slug: 'dogs', total: 40 },
	{ slug: 'cakes', total: 20 }
];

describe('the set a binder opens on', () => {
	it('is the one closest to complete', () => {
		expect(defaultSet(sets, { gems: 2, ships: 100, dogs: 10 })).toBe('ships');
	});
	it('prefers a completed set, the biggest of them', () => {
		expect(defaultSet(sets, { gems: 10, ships: 149, cakes: 20 })).toBe('cakes');
		expect(defaultSet(sets, { gems: 10, ships: 150, cakes: 20 })).toBe('ships');
	});
	it('falls back to the first set for a new player', () => {
		expect(defaultSet(sets, {})).toBe('gems');
		expect(defaultSet([], {})).toBeUndefined();
	});
});
