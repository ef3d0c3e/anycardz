import type { I18n } from './i18n/index.svelte.ts';
import { featHref, featLine } from './progress.ts';
import { missionLine } from './missions.ts';
import type { CatalogState } from './session.svelte.ts';
import type { MarketSoldDetail, NotificationDto } from './types.ts';

/** What a market sale notification carries, or null when it is unreadable */
export function soldDetail(n: Pick<NotificationDto, 'detail'>): MarketSoldDetail | null {
	try {
		const d = JSON.parse(n.detail ?? '');
		return typeof d?.net === 'number' ? d : null;
	} catch {
		return null;
	}
}

/** The line a notification reads as, in the bell and in toasts */
export function notifText(
	i18n: I18n,
	catalog: CatalogState | undefined,
	n: Pick<NotificationDto, 'kind' | 'actor' | 'detail'>
): string {
	const name = n.actor ?? '?';
	if (n.kind === 'achievement') return featLine(i18n, catalog, n.detail ?? '');
	if (n.kind === 'mission') return missionLine(i18n, n.detail ?? '');
	if (n.kind === 'market_sold')
		return i18n.t('notif.market_sold', { name, n: soldDetail(n)?.net ?? 0 });
	return i18n.t(`notif.${n.kind}`, { name });
}

/** Where a notification leads */
export function notifHref(n: Pick<NotificationDto, 'kind' | 'actor' | 'detail' | 'trade'>): string {
	if (n.kind === 'achievement') return featHref(n.detail ?? '');
	if (n.kind === 'market_sold') return '/market#selling';
	if (n.kind === 'mission') return n.detail?.startsWith('bonus') ? '/open' : '/';
	if (n.kind === 'friend_request') return '/friends';
	if (n.kind === 'friend_accepted') return `/players/${encodeURIComponent(n.actor ?? '')}`;
	return n.trade ? `/trade/${n.trade}` : '/trade';
}
