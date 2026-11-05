import { getContext, setContext } from 'svelte';
import {
	FREE_PACK,
	type CollectionDto,
	type FreePacksDto,
	type MeDto,
	type MissionsDto,
	type TicketsDto,
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
	missions = $state<MissionsDto>() as MissionsDto;
	tickets = $state<TicketsDto>() as TicketsDto;

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
		this.missions = me.missions;
		this.tickets = me.tickets;
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
			set_reward: 0,
			foil_set_reward: 0,
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
	/** The same message again while it shows: "×3" instead of three toasts */
	count: number;
}

export class Toasts {
	/** On screen: two at most, so a burst (a pack unlocking several achievements) never floods it */
	list = $state<Toast[]>([]);
	/** Phones: above the bottom bar, or under the header where a page keeps its controls low */
	placement = $state<'bottom' | 'top'>('bottom');
	#waiting: Toast[] = [];
	#timers = new Map<number, ReturnType<typeof setTimeout>>();
	#next = 0;
	static readonly VISIBLE = 2;
	static readonly MS = 3400;
	/** With others waiting, each stays less, so a queue drains in seconds */
	static readonly BUSY_MS = 1600;
	static readonly MAX_WAITING = 4;
	show = (text: string, kind: Toast['kind'] = '') => {
		const same = (t: Toast) => t.text === text && t.kind === kind;
		const shown = this.list.find(same);
		if (shown) {
			this.list = this.list.map((t) => (t === shown ? { ...t, count: t.count + 1 } : t));
			this.#schedule(shown.id);
			return;
		}
		const queued = this.#waiting.find(same);
		if (queued) {
			queued.count++;
			return;
		}
		this.#waiting.push({ id: this.#next++, text, kind, count: 1 });
		// A long queue loses its oldest news, never a warning
		while (this.#waiting.length > Toasts.MAX_WAITING) {
			const i = this.#waiting.findIndex((t) => t.kind !== 'warn');
			this.#waiting.splice(i < 0 ? 0 : i, 1);
		}
		this.#pump();
		// Waiting news hurries the ones on screen
		if (this.#waiting.length) for (const t of this.list) this.#schedule(t.id, Toasts.BUSY_MS);
	};
	#schedule(id: number, ms = this.#waiting.length ? Toasts.BUSY_MS : Toasts.MS) {
		clearTimeout(this.#timers.get(id));
		this.#timers.set(
			id,
			setTimeout(() => {
				this.#timers.delete(id);
				this.list = this.list.filter((x) => x.id !== id);
				this.#pump();
			}, ms)
		);
	}
	/** The waiting ones take the free places, in order */
	#pump() {
		while (this.list.length < Toasts.VISIBLE && this.#waiting.length) {
			const t = this.#waiting.shift()!;
			this.list = [...this.list, t];
			this.#schedule(t.id);
		}
	}
}
const TOASTS = Symbol('toasts');
export const setToasts = () => setContext(TOASTS, new Toasts());
export const getToasts = () => getContext<Toasts>(TOASTS);
