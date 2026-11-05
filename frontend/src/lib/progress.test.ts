import { describe, expect, it } from 'vitest';
import { featHref, parseKey } from './progress.ts';

describe('achievement keys', () => {
	it('tells sets from foil sets from the rest', () => {
		expect(parseKey('set:desserts')).toEqual({ kind: 'set', id: 'desserts' });
		expect(parseKey('foilset:ww2-warships')).toEqual({ kind: 'foilset', id: 'ww2-warships' });
		expect(parseKey('first_trade')).toEqual({ kind: 'other', id: 'first_trade' });
		expect(parseKey('set:')).toEqual({ kind: 'other', id: 'set:' });
	});
	it('leads sets to their binder page', () => {
		expect(featHref('set:desserts')).toBe('/binder?set=desserts');
		expect(featHref('foilset:a b')).toBe('/binder?set=a%20b');
		expect(featHref('first_trade')).toBe('/achievements#ach-first_trade');
	});
});
