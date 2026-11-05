import { describe, expect, it } from 'vitest';
import { change, diff, isEmpty, onSide, type Line } from './offer.ts';

const l = (card: string, count = 1, foil = false): Line => ({ card, foil, count });

describe('change', () => {
	it('adds, increments and removes lines', () => {
		let side = change([], 'ak', false, 1);
		expect(side).toEqual([l('ak')]);
		side = change(side, 'ak', false, 2);
		expect(side).toEqual([l('ak', 3)]);
		side = change(side, 'ak', true, 1);
		expect(onSide(side, 'ak', true)).toBe(1);
		side = change(side, 'ak', false, -3);
		expect(side).toEqual([l('ak', 1, true)]);
	});
	it('never goes below zero or edits in place', () => {
		const before = [l('ak')];
		expect(change(before, 'mp5', false, -1)).toEqual(before);
		change(before, 'ak', false, 5);
		expect(before).toEqual([l('ak')]);
	});
});

describe('diff', () => {
	it('spots added and removed copies, foils apart', () => {
		const d = diff([l('ak', 2), l('bismarck')], [l('ak', 1), l('bismarck', 1, true), l('mp5')]);
		expect(d.added).toEqual([l('bismarck', 1, true), l('mp5')]);
		expect(d.removed).toEqual([l('ak'), l('bismarck')]);
	});
	it('is empty when nothing changed', () => {
		expect(isEmpty(diff([l('ak', 2)], [l('ak', 2)]))).toBe(true);
	});
});
