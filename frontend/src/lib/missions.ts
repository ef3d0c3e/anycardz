import type { I18n } from './i18n/index.svelte.ts';

/** Goals of the missions, as the API sets them: names say "Open 3 packs" */
export const GOALS: Record<string, number> = {
	open_packs: 3,
	pull_rare: 2,
	tradeup: 1,
	roulette: 1,
	trade: 1,
	market_list: 1,
	market_buy: 1,
	shop_sell: 3,
	free_packs: 1,
	scratch: 3
};

/** Where each mission is done */
export const WHERE: Record<string, string> = {
	open_packs: '/open',
	pull_rare: '/open',
	tradeup: '/forge',
	roulette: '/forge?tab=roulette',
	trade: '/trade',
	market_list: '/binder',
	market_buy: '/market',
	shop_sell: '/binder',
	free_packs: '/',
	scratch: '/open'
};

export function missionName(i18n: I18n, key: string, goal = GOALS[key] ?? 1): string {
	return i18n.has(`mission.${key}`) ? i18n.t(`mission.${key}` as never, { n: goal }) : key;
}

/** A mission notification: `key:coins`, or `bonus:0` for all three */
export function missionLine(i18n: I18n, detail: string): string {
	const [key, coins] = detail.split(':');
	if (key === 'bonus') return i18n.t('mission.bonus_done');
	return i18n.t('mission.done', { name: missionName(i18n, key), n: Number(coins) || 0 });
}
