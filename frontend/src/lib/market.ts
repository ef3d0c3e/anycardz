import type { Rarity } from './types.ts';

/** What the shop pays for a copy: the lowest price a listing can ask */
export const SELL_PRICE: Record<Rarity, number> = { common: 10, rare: 50, legendary: 250 };
export const MAX_FACTOR = 10;
export const FEE_PERCENT = 5;

/** Prices a copy can be listed at, as the API checks them */
export function bounds(rarity: Rarity, foil: boolean): { min: number; max: number } {
	const min = SELL_PRICE[rarity] * (foil ? 2 : 1);
	return { min, max: min * MAX_FACTOR };
}

/** The fee kept from a sale: 5 %, rounded up, at least 1 */
export const fee = (price: number) => Math.max(1, Math.ceil((price * FEE_PERCENT) / 100));

/** What the seller gets */
export const net = (price: number) => price - fee(price);
