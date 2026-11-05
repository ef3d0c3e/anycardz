import { describe, expect, it } from 'vitest';
import { notifHref, soldDetail } from './notify.ts';

describe('notifications', () => {
	it('reads what a sale carries', () => {
		const detail = JSON.stringify({ card: 'quartz', foil: false, price: 50, net: 47 });
		expect(soldDetail({ detail })?.net).toBe(47);
		expect(soldDetail({ detail: 'nope' })).toBeNull();
		expect(soldDetail({ detail: null })).toBeNull();
	});
	it('leads each kind somewhere', () => {
		const base = { actor: 'bob', detail: null, trade: null };
		expect(notifHref({ ...base, kind: 'market_sold' })).toBe('/market#selling');
		expect(notifHref({ ...base, kind: 'trade_done', trade: 4 })).toBe('/trade/4');
		expect(notifHref({ ...base, kind: 'achievement', detail: 'set:gems' })).toBe(
			'/binder?set=gems'
		);
	});
});
