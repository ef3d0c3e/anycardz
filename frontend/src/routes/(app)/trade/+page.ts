import { api } from '#lib/api.ts';
import type { TradeSummary } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => ({
	trades: await api<TradeSummary[]>('/trades', {}, fetch)
});
