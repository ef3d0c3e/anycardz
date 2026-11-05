// Which boosters the lobby shelf shows, and in what order.

export interface ShelfSet {
	slug: string;
	number: number;
	total: number;
}

/** Sets on the shelf; the rest is one tap away in the set picker */
export const SHELF_MAX = 10;
export const RECENT_KEY = 'anycardz-recent-sets';

/**
 * Recently picked sets first, then sets in progress (closest to complete
 * first), then untouched ones by set number, completed sets last.
 */
export function shelfOrder<T extends ShelfSet>(
	sets: T[],
	owned: Record<string, number>,
	recent: string[],
	max = SHELF_MAX
): T[] {
	const rank = (s: T) => {
		const n = owned[s.slug] ?? 0;
		return n >= s.total ? 2 : n > 0 ? 0 : 1;
	};
	const pct = (s: T) => (owned[s.slug] ?? 0) / s.total;
	const picked = recent.map((slug) => sets.find((s) => s.slug === slug)).filter((s): s is T => !!s);
	const rest = sets
		.filter((s) => !recent.includes(s.slug))
		.toSorted((a, b) => rank(a) - rank(b) || pct(b) - pct(a) || a.number - b.number);
	return [...picked, ...rest].slice(0, max);
}

export function readRecent(): string[] {
	try {
		const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
		return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [];
	} catch {
		return [];
	}
}
