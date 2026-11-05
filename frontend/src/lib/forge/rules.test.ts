import { describe, expect, it } from 'vitest';
import { needleAngle, ROULETTE_CAP, rouletteChance, spare, tradeupOdds, wedge } from './rules.ts';

describe('roulette chance', () => {
	// Same cases as backend/src/game.rs roulette_chances_match_the_old_rules
	it('matches the server', () => {
		expect(rouletteChance('common', [])).toBe(100);
		expect(rouletteChance('legendary', [])).toBe(500);
		expect(rouletteChance('rare', ['common', 'rare', 'legendary'])).toBe(200 + 50 + 100 + 200);
		expect(rouletteChance('common', Array(98).fill('common'))).toBe(ROULETTE_CAP);
		expect(rouletteChance('legendary', Array(100).fill('legendary'))).toBe(ROULETTE_CAP);
	});
});

describe('spare copies', () => {
	it('keeps one copy of every card', () => {
		expect(spare({ card: 'a', count: 1, foils: 0 })).toBe(0);
		expect(spare({ card: 'a', count: 4, foils: 0 })).toBe(3);
		expect(spare({ card: 'a', count: 4, foils: 0 }, 2)).toBe(1);
	});
	it('a foil is the kept copy, and is never spent', () => {
		expect(spare({ card: 'a', count: 3, foils: 1 })).toBe(2);
		expect(spare({ card: 'a', count: 2, foils: 2 })).toBe(0);
		expect(spare(undefined)).toBe(0);
	});
});

describe('trade-up odds', () => {
	const cols = [
		{ slug: 'gems', rarities: { common: 5, rare: 2 } },
		{ slug: 'dogs', rarities: { common: 5, rare: 1 } },
		{ slug: 'nope', rarities: { common: 5 } }
	];
	const slots = (...c: string[]) => c.map((collection) => ({ collection }));
	it('weighs collections by the cards they put in', () => {
		expect(tradeupOdds(slots('gems', 'gems', 'gems', 'dogs'), 'rare', cols)).toEqual([
			{ collection: 'gems', share: 0.75 },
			{ collection: 'dogs', share: 0.25 }
		]);
	});
	it('leaves out collections with nothing to pay out', () => {
		expect(tradeupOdds(slots('gems', 'nope'), 'rare', cols)).toEqual([
			{ collection: 'gems', share: 1 }
		]);
		expect(tradeupOdds(slots('nope'), 'rare', cols)).toEqual([]);
	});
});

describe('needle', () => {
	const span = (bp: number) => (bp / 10_000) * 360;
	it('lands inside the slice on a win, outside on a loss', () => {
		for (const bp of [100, 700, 2500, 5000]) {
			for (const r of [0, 0.5, 0.999]) {
				const win = needleAngle(true, bp, () => r);
				expect(win).toBeGreaterThan(0);
				expect(win).toBeLessThan(span(bp));
				const loss = needleAngle(false, bp, () => r);
				expect(loss).toBeGreaterThan(span(bp));
				expect(loss).toBeLessThan(360);
			}
		}
	});
	it('draws the slice', () => {
		expect(wedge(0)).toBe('');
		expect(wedge(25)).toBe('M0 0L0 -96A96 96 0 0 1 96.00 -0.00Z');
		expect(wedge(75)).toContain(' 0 1 1 ');
	});
});
