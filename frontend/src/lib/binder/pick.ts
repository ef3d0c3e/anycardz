import type { CollectionDto } from '#lib/types.ts';

/**
 * The set a binder opens on: the one closest to complete. Completed sets
 * count as the closest, and between them the biggest wins (it took the most
 * to finish). A player who owns nothing gets the first set.
 */
export function defaultSet(
	collections: Pick<CollectionDto, 'slug' | 'total'>[],
	owned: Record<string, number>
): string | undefined {
	let best: { slug: string; ratio: number; total: number } | undefined;
	for (const c of collections) {
		const have = owned[c.slug] ?? 0;
		if (!have || !c.total) continue;
		const ratio = Math.min(1, have / c.total);
		if (!best || ratio > best.ratio || (ratio === best.ratio && c.total > best.total))
			best = { slug: c.slug, ratio, total: c.total };
	}
	return best?.slug ?? collections[0]?.slug;
}
