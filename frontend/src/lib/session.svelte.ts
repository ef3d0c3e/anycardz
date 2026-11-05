import { getContext, setContext } from 'svelte';
import type { CollectionDto, MeDto, StreakDto } from './types';

/** The logged-in player, kept in sync with API responses */
export class Session {
	username = $state('');
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
	constructor(c: CollectionDto[]) {
		this.collections = c;
	}
	get(slug: string) {
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
