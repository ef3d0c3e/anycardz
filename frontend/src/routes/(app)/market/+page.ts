import { api } from '#lib/api.ts';
import type { MarketMine, MarketPage } from '#lib/types.ts';
import type { PageLoad } from './$types';

/** The URL holds the filters: a search can be shared and survives a reload */
export const load: PageLoad = async ({ fetch, url, depends }) => {
	depends('app:market');
	const q = new URLSearchParams();
	for (const k of ['q', 'card', 'rarity', 'foil', 'sort']) {
		const v = url.searchParams.get(k);
		if (v) q.set(k, v);
	}
	const [market, mine] = await Promise.all([
		api<MarketPage>(`/market?${q}`, {}, fetch),
		api<MarketMine>('/market/mine', {}, fetch)
	]);
	return { market, mine, filters: Object.fromEntries(q) as Record<string, string> };
};
