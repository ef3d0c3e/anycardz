import { getContext, setContext } from 'svelte';
import { api } from './api.ts';
import type { CardDto, ChatEvent, NotificationDto, PullEvent } from './types.ts';

type LiveMessage =
	| { type: 'hello'; pulls: PullEvent[]; messages: ChatEvent[] }
	| ({ type: 'pull' } & PullEvent)
	| ({ type: 'chat' } & ChatEvent)
	| { type: 'trade'; id: number }
	| ({ type: 'notification' } & NotificationDto);

const MAX_PULLS = 30;
const MAX_MESSAGES = 100;

/**
 * The live socket: rare pulls, the shoutbox, trade pings. Push only, sending
 * goes through the REST API. Reconnects on its own after a drop.
 */
export class Live {
	pulls = $state<PullEvent[]>([]);
	messages = $state<ChatEvent[]>([]);
	connected = $state(false);
	/** Messages that arrived while the panel was closed */
	unread = $state(0);
	panelOpen = $state(false);
	/** Card details by slug, in the current language */
	cards = $state<Record<string, CardDto>>({});
	/** Players the user blocked: their messages and pulls are left out */
	blocked = $state<Set<string>>(new Set());

	#ws: WebSocket | null = null;
	#retry = 1000;
	#closed = false;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#missing = new Set<string>();
	#tradeListeners = new Set<(id: number) => void>();
	#noteListeners = new Set<(n: NotificationDto) => void>();

	start() {
		this.#closed = false;
		this.#connect();
		this.loadBlocks();
	}

	/** Refreshes the block list, and drops what blocked players already posted */
	async loadBlocks() {
		const list = await api<{ username: string }[]>('/blocks').catch(() => null);
		if (!list) return;
		this.blocked = new Set(list.map((b) => b.username));
		this.messages = this.messages.filter((m) => !this.blocked.has(m.user));
		this.pulls = this.pulls.filter((p) => !this.blocked.has(p.user));
	}

	stop() {
		this.#closed = true;
		clearTimeout(this.#timer);
		this.#ws?.close();
	}

	#connect() {
		const proto = location.protocol === 'https:' ? 'wss' : 'ws';
		const ws = new WebSocket(`${proto}://${location.host}/api/live`);
		this.#ws = ws;
		ws.onopen = () => {
			this.connected = true;
			this.#retry = 1000;
		};
		ws.onmessage = (e) => this.#receive(JSON.parse(e.data) as LiveMessage);
		ws.onclose = () => {
			this.connected = false;
			if (this.#closed) return;
			// Back off up to 30 s, so a server restart is not hammered
			this.#timer = setTimeout(() => this.#connect(), this.#retry);
			this.#retry = Math.min(this.#retry * 2, 30_000);
		};
	}

	#receive(m: LiveMessage) {
		switch (m.type) {
			case 'hello':
				this.pulls = m.pulls.filter((p) => !this.blocked.has(p.user));
				this.messages = m.messages;
				this.#need([
					...m.pulls.map((p) => p.card),
					...m.messages.flatMap((c) => (c.card ? [c.card] : []))
				]);
				break;
			case 'pull': {
				const { type: _, ...p } = m;
				if (this.blocked.has(p.user)) break;
				this.pulls = [p, ...this.pulls].slice(0, MAX_PULLS);
				this.#need([p.card]);
				break;
			}
			case 'chat': {
				const { type: _, ...c } = m;
				if (this.blocked.has(c.user) || this.messages.some((x) => x.id === c.id)) break;
				this.messages = [...this.messages, c].slice(-MAX_MESSAGES);
				if (!this.panelOpen) this.unread++;
				if (c.card) this.#need([c.card]);
				break;
			}
			case 'trade':
				for (const f of this.#tradeListeners) f(m.id);
				break;
			case 'notification': {
				const { type: _, ...n } = m;
				for (const f of this.#noteListeners) f(n);
				break;
			}
		}
	}

	/** Called with a trade id whenever one of the player's trades changes */
	onTrade(f: (id: number) => void) {
		this.#tradeListeners.add(f);
		return () => this.#tradeListeners.delete(f);
	}

	/** Called with every new notification for the player */
	onNotification(f: (n: NotificationDto) => void) {
		this.#noteListeners.add(f);
		return () => this.#noteListeners.delete(f);
	}

	/** A message the player just sent, before the socket echoes it */
	sent(c: ChatEvent) {
		if (!this.messages.some((x) => x.id === c.id))
			this.messages = [...this.messages, c].slice(-MAX_MESSAGES);
		if (c.card) this.#need([c.card]);
	}

	/** Fetches card details for slugs we have not seen yet, in batches */
	#need(slugs: string[]) {
		for (const s of slugs) if (!this.cards[s]) this.#missing.add(s);
		if (!this.#missing.size) return;
		queueMicrotask(async () => {
			const batch = [...this.#missing].slice(0, 100);
			if (!batch.length) return;
			batch.forEach((s) => this.#missing.delete(s));
			const found = await api<Record<string, CardDto>>(
				`/cards?slugs=${batch.map(encodeURIComponent).join(',')}`
			).catch(() => ({}));
			this.cards = { ...this.cards, ...found };
			if (this.#missing.size) this.#need([]);
		});
	}

	/** After a language switch: card texts again, in the new language */
	relocalize() {
		const slugs = Object.keys(this.cards);
		this.cards = {};
		this.#need(slugs);
	}
}

const KEY = Symbol('live');
export const setLive = () => setContext(KEY, new Live());
export const getLive = () => getContext<Live>(KEY);

/** "3 min ago", in the page language */
export function ago(iso: string, lang: string, now = Date.now()): string {
	const s = Math.round((new Date(iso).getTime() - now) / 1000);
	const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' });
	if (Math.abs(s) < 45) return rtf.format(0, 'second');
	if (Math.abs(s) < 3600) return rtf.format(Math.round(s / 60), 'minute');
	if (Math.abs(s) < 86_400) return rtf.format(Math.round(s / 3600), 'hour');
	return rtf.format(Math.round(s / 86_400), 'day');
}
