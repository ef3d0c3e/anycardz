import { api } from '#lib/api.ts';
import { BOARDS, type Board, type LeaderboardDto, type Period } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const b = url.searchParams.get('board');
	const board: Board = (BOARDS as readonly string[]).includes(b ?? '') ? (b as Board) : 'sets';
	// Streaks only count all time
	const period: Period =
		board !== 'streak' && url.searchParams.get('period') === 'week' ? 'week' : 'all';
	return {
		board,
		period,
		ranking: await api<LeaderboardDto>(`/leaderboards?board=${board}&period=${period}`, {}, fetch)
	};
};
