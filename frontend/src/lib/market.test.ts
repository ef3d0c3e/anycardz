import { describe, expect, it } from 'vitest';
import { bounds, fee, net } from './market.ts';

describe('market rules', () => {
	it('bounds prices from the shop price to ten times it', () => {
		expect(bounds('common', false)).toEqual({ min: 10, max: 100 });
		expect(bounds('legendary', true)).toEqual({ min: 500, max: 5000 });
	});
	it('rounds the fee up, like the API', () => {
		expect([fee(10), fee(100), fee(101), fee(5000)]).toEqual([1, 5, 6, 250]);
		expect(net(50)).toBe(47);
	});
});
