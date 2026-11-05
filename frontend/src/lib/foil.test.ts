import { describe, expect, it } from 'vitest';
import { foilOf } from './foil.ts';

describe('foilOf', () => {
	it('gives the real pattern of each foil copy', () => {
		const o = { foils: 2, patterns: ['foil5', 'foil1'] };
		expect(foilOf(o)).toBe('foil5');
		expect(foilOf(o, 1)).toBe('foil1');
		expect(foilOf(o, 2)).toBeNull();
	});
	it('is null without foils', () => {
		expect(foilOf({ foils: 0, patterns: [] })).toBeNull();
		expect(foilOf(undefined)).toBeNull();
	});
});
