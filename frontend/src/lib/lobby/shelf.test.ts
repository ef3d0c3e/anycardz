import { describe, expect, it } from 'vitest';
import { SHELF_MAX, shelfOrder } from './shelf.ts';

const set = (slug: string, number: number, total = 100) => ({ slug, number, total });

describe('shelf order', () => {
	const sets = [set('a', 1), set('b', 2), set('c', 3), set('d', 4, 10), set('e', 5)];
	it('puts sets in progress first, most complete on top, finished sets last', () => {
		const owned = { b: 10, c: 60, d: 10 };
		expect(shelfOrder(sets, owned, []).map((s) => s.slug)).toEqual(['c', 'b', 'a', 'e', 'd']);
	});
	it('puts recently picked sets in front, in pick order', () => {
		expect(shelfOrder(sets, { c: 60 }, ['e', 'a']).map((s) => s.slug)).toEqual([
			'e',
			'a',
			'c',
			'b',
			'd'
		]);
	});
	it('ignores picks of sets that no longer exist', () => {
		expect(shelfOrder(sets, {}, ['gone', 'b'])[0].slug).toBe('b');
	});
	it('keeps the shelf short with many sets', () => {
		const many = Array.from({ length: 80 }, (_, i) => set(`s${i}`, i + 1));
		const shelf = shelfOrder(many, { s79: 5 }, ['s40']);
		expect(shelf).toHaveLength(SHELF_MAX);
		expect(shelf.slice(0, 3).map((s) => s.slug)).toEqual(['s40', 's79', 's0']);
	});
});
