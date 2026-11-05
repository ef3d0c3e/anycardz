import type { I18n } from './i18n/index.svelte.ts';
import type { CatalogState } from './session.svelte.ts';

/** What an achievement key is about. Sets: `set:<slug>`, `foilset:<slug>`. */
export function parseKey(key: string): { kind: 'set' | 'foilset' | 'other'; id: string } {
	const [head, ...rest] = key.split(':');
	const id = rest.join(':');
	if ((head === 'set' || head === 'foilset') && id) return { kind: head, id };
	return { kind: 'other', id: key };
}

/** "Minerals" or "Minerals (foil)": the name of what was earned */
export function featName(i18n: I18n, catalog: CatalogState | undefined, key: string): string {
	const { kind, id } = parseKey(key);
	if (kind === 'other') return i18n.has(`ach.${id}`) ? i18n.t(`ach.${id}` as never) : id;
	const set = catalog?.get(id)?.title ?? id;
	return kind === 'foilset' ? i18n.t('feat.foil_set_name', { set }) : set;
}

/**
 * One line telling what someone earned, for the live feed and notifications.
 * `user` is who did it; `you` says it was the viewer, with its own wording
 * ("Tu as complété…", not a name in the third person).
 */
export function featLine(
	i18n: I18n,
	catalog: CatalogState | undefined,
	key: string,
	user?: string,
	you = false
): string {
	const { kind } = parseKey(key);
	const name = featName(i18n, catalog, key);
	const what = kind === 'other' ? 'feat.achievement' : 'feat.completed';
	if (you) return i18n.t(`${what}_you`, { name });
	return user ? i18n.t(`${what}_by`, { user, name }) : i18n.t(what, { name });
}

/** Where an earned key leads */
export function featHref(key: string): string {
	const { kind, id } = parseKey(key);
	return kind === 'other' ? '/achievements' : `/binder?set=${encodeURIComponent(id)}`;
}
