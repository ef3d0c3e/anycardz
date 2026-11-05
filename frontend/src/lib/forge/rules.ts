// Forge rules shown before the server decides. They mirror backend/src/game.rs:
// the tests pin the same numbers, so the dial never promises other odds.
import type { CollectionDto, OwnedDto, Rarity } from '#lib/types.ts';

export const TRADEUP_COST = 10;
export const ROULETTE_CAP = 5000;
export const ROULETTE_MAX_BURN = 100;
export const ABOVE: Partial<Record<Rarity, Rarity>> = { common: 'rare', rare: 'legendary' };

const ODDS: Record<Rarity, [base: number, bonus: number]> = {
	common: [100, 50],
	rare: [200, 100],
	legendary: [500, 200]
};

/** Chance in basis points (1/100 of a percent) */
export function rouletteChance(target: Rarity, burned: Rarity[]): number {
	return Math.min(ROULETTE_CAP, ODDS[target][0] + burned.reduce((s, r) => s + ODDS[r][1], 0));
}
export const rouletteBase = (r: Rarity) => ODDS[r][0];
export const rouletteBonus = (r: Rarity) => ODDS[r][1];

/**
 * Copies of a card the forge may spend: plain copies, keeping one copy of the
 * card overall (a foil counts as the kept copy).
 */
export function spare(o: OwnedDto | undefined, used = 0): number {
	if (!o) return 0;
	const plain = o.count - o.foils;
	const keep = o.foils > 0 ? 0 : 1;
	return Math.max(0, plain - keep - used);
}

/** Odds per collection of a trade-up: share of the cards it put in, among collections that can pay out */
export function tradeupOdds(
	slots: { collection: string }[],
	target: Rarity,
	collections: Pick<CollectionDto, 'slug' | 'rarities'>[]
): { collection: string; share: number }[] {
	const weights = new Map<string, number>();
	for (const s of slots) {
		const c = collections.find((x) => x.slug === s.collection);
		if ((c?.rarities?.[target] ?? 0) > 0)
			weights.set(s.collection, (weights.get(s.collection) ?? 0) + 1);
	}
	const total = [...weights.values()].reduce((a, b) => a + b, 0);
	return [...weights]
		.map(([collection, w]) => ({ collection, share: w / total }))
		.sort((a, b) => b.share - a.share || a.collection.localeCompare(b.collection));
}

/** Where the needle stops: inside the win slice [0, chance) for a win, outside for a loss */
export function needleAngle(won: boolean, chanceBp: number, random = Math.random): number {
	const span = (chanceBp / 10_000) * 360;
	if (won) return span * (0.15 + random() * 0.7);
	const margin = Math.min(3, (360 - span) / 4);
	return span + margin + random() * (360 - span - 2 * margin);
}

/** SVG path of the win slice of a dial of radius r, starting at 12 o'clock */
export function wedge(pct: number, r = 96): string {
	if (pct <= 0) return '';
	if (pct >= 100) return `M0 ${-r}A${r} ${r} 0 1 1 0 ${r}A${r} ${r} 0 1 1 0 ${-r}Z`;
	const a = (pct / 100) * Math.PI * 2;
	const x = (Math.sin(a) * r).toFixed(2);
	const y = (-Math.cos(a) * r).toFixed(2);
	return `M0 0L0 ${-r}A${r} ${r} 0 ${pct > 50 ? 1 : 0} 1 ${x} ${y}Z`;
}
