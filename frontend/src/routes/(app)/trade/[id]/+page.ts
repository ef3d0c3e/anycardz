import { error } from '@sveltejs/kit';
import { api, ApiError } from '#lib/api.ts';
import type { BinderDto, PlayerCardsDto, TradeDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params }) => {
	let trade: TradeDto;
	try {
		trade = await api<TradeDto>(`/trades/${params.id}`, {}, fetch);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'unknown_trade');
		throw e;
	}
	const [mine, theirs] = await Promise.all([
		api<BinderDto>('/binder?details=1', {}, fetch),
		api<PlayerCardsDto>(`/players/${encodeURIComponent(trade.partner)}/cards`, {}, fetch)
	]);
	return { trade, mine, theirs };
};
