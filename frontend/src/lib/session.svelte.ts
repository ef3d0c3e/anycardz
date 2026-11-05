import { getContext, setContext } from 'svelte';
import {
	FREE_PACK,
	type CollectionDto,
	type FreePacksDto,
	type MeDto,
	type StreakDto
} from './types';

/** The logged-in player, kept in sync with API responses */
export class Session {
	username = $state('');
	email = $state('');
	termsOutdated = $state(false);
	freePacks = $state<FreePacksDto>() as FreePacksDto;
	pityLeft = $state(0);
	avatar = $state<string | null>(null);
	friendRequests = $state(0);
	notifications = $state(0);
	coins = $state(0);
	pile = $state<string[]>([]);
	streak = $state<StreakDto>() as StreakDto;

	constructor(me: MeDto) {
		this.update(me);
	}
	update(me: MeDto) {
		this.username = me.username;
		this.email = me.email;
		this.termsOutdated = me.terms_outdated;
		this.freePacks = me.free_packs;
		this.pityLeft = me.pity_left;
		this.avatar = me.avatar;
		this.friendRequests = me.friend_requests;
		this.notifications = me.notifications;
		this.coins = me.coins;
		this.pile = me.pile;
		this.streak = me.streak;
	}
}

/** Collection summaries in the current language */
export class CatalogState {
	collections = $state<CollectionDto[]>([]);
	/** Name of the free pack in the current language */
	freeTitle = $state('Free pack');
	constructor(c: CollectionDto[]) {
		this.collections = c;
	}
	/** A stand-in collection for free packs, drawn from all the others */
	free = $derived.by((): CollectionDto | undefined => {
		const all = this.collections;
		if (!all.length) return undefined;
		const featured =
			all.find((c) => c.featured.rarity === 'legendary')?.featured ?? all[0].featured;
		return {
			slug: FREE_PACK,
			number: 0,
			title: this.freeTitle,
			description: '',
			primary: '#6d4cff',
			secondary: '#ff4fa3',
			total: all.reduce((n, c) => n + c.total, 0),
			price: 0,
			featured,
			free: true
		};
	});
	get(slug: string) {
		if (slug === FREE_PACK) return this.free;
		return this.collections.find((c) => c.slug === slug);
	}
}

const SESSION = Symbol('session');
const CATALOG = Symbol('catalog');
export const setSession = (me: MeDto) => setContext(SESSION, new Session(me));
export const getSession = () => getContext<Session>(SESSION);
export const setCatalog = (c: CollectionDto[]) => setContext(CATALOG, new CatalogState(c));
export const getCatalog = () => getContext<CatalogState | undefined>(CATALOG);

interface Toast {
	id: number;
	text: string;
	kind: '' | 'warn';
}
export class Toasts {
	list = $state<Toast[]>([]);
	#next = 0;
	show = (text: string, kind: Toast['kind'] = '') => {
		const id = this.#next++;
		this.list.push({ id, text, kind });
		setTimeout(() => (this.list = this.list.filter((t) => t.id !== id)), 3400);
	};
}
const TOASTS = Symbol('toasts');
export const setToasts = () => setContext(TOASTS, new Toasts());
export const getToasts = () => getContext<Toasts>(TOASTS);
