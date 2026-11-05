<script lang="ts">
	import { goto } from '$app/navigation';
	import { tick } from 'svelte';
	import { getI18n, type TKey } from '#lib/i18n/index.svelte.ts';
	import { getSession } from '#lib/session.svelte.ts';
	import { api, artUrl } from '#lib/api.ts';
	import type { CardDto } from '#lib/types.ts';
	import Avatar from './Avatar.svelte';

	/** Ctrl+K: pages, cards, sets and players in one list */
	let { open = $bindable(false) }: { open?: boolean } = $props();
	const i18n = getI18n();
	const session = getSession();

	interface Hit {
		group: 'pages' | 'cards' | 'sets' | 'players';
		label: string;
		sub?: string;
		href: string;
		img?: string;
		avatar?: { name: string; src: string | null };
		swatch?: string;
	}
	const PAGES: [TKey, string][] = [
		['nav.lobby', '/'],
		['nav.binder', '/binder'],
		['nav.open', '/open'],
		['nav.forge', '/forge'],
		['nav.trade', '/trade'],
		['menu.friends', '/friends'],
		['menu.profile', '/me']
	];

	let q = $state('');
	let hits = $state<Hit[]>([]);
	let sel = $state(0);
	let input = $state<HTMLInputElement>();
	let list = $state<HTMLUListElement>();
	let timer: ReturnType<typeof setTimeout>;
	let seq = 0;

	const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
	const pages = $derived(
		PAGES.filter(([k]) => !q.trim() || fold(i18n.t(k)).includes(fold(q.trim()))).map(
			([k, href]): Hit => ({ group: 'pages', label: i18n.t(k), href })
		)
	);

	$effect(() => {
		if (!open) return;
		q = '';
		sel = 0;
		tick().then(() => input?.focus());
	});
	$effect(() => {
		const query = q.trim();
		clearTimeout(timer);
		sel = 0;
		if (query.length < 2) {
			hits = [];
			return;
		}
		const mine = ++seq;
		timer = setTimeout(async () => {
			const r = await api<{
				cards: CardDto[];
				collections: { slug: string; title: string; primary: string }[];
				players: { username: string; avatar: string | null }[];
			}>(`/search?q=${encodeURIComponent(query)}`).catch(() => null);
			// An older, slower answer never replaces a newer one
			if (!r || mine !== seq) return;
			hits = [
				...r.cards.map((c): Hit => ({
					group: 'cards',
					label: c.title,
					sub: `${i18n.t(`card.${c.rarity}`)} · #${c.number}`,
					href: `/binder?set=${c.collection}&card=${c.slug}`,
					img: artUrl(c.slug)
				})),
				...r.collections.map((c): Hit => ({
					group: 'sets',
					label: c.title,
					href: `/binder?set=${c.slug}`,
					swatch: c.primary
				})),
				...r.players.map((p): Hit => ({
					group: 'players',
					label: p.username,
					href: `/players/${encodeURIComponent(p.username)}`,
					avatar: { name: p.username, src: p.avatar }
				}))
			];
		}, 150);
	});
	const all = $derived([...pages, ...hits]);

	function go(h: Hit | undefined) {
		if (!h) return;
		open = false;
		goto(h.href);
	}
	function keys(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			sel = (sel + (e.key === 'ArrowDown' ? 1 : -1) + all.length) % Math.max(1, all.length);
			tick().then(() =>
				list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' })
			);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			go(all[sel]);
		} else if (e.key === 'Escape') open = false;
	}
	const groupName = {
		pages: 'search.pages',
		cards: 'search.cards',
		sets: 'search.sets',
		players: 'search.players'
	} as const;
</script>

<svelte:window
	onkeydown={(e) => {
		if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
			e.preventDefault();
			open = !open;
		}
	}}
/>

{#if open}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="scrim" onclick={(e) => e.target === e.currentTarget && (open = false)}>
		<div class="pal" role="dialog" aria-modal="true" aria-label={i18n.t('search.label')}>
			<input
				bind:this={input}
				bind:value={q}
				id="palette-input"
				type="search"
				role="combobox"
				aria-expanded="true"
				aria-controls="palette-list"
				aria-activedescendant={all.length ? `hit-${sel}` : undefined}
				placeholder={i18n.t('search.placeholder')}
				aria-label={i18n.t('search.label')}
				autocomplete="off"
				onkeydown={keys}
			/>
			<ul id="palette-list" role="listbox" bind:this={list}>
				{#each all as h, i (`${h.group}-${h.href}`)}
					{#if i === 0 || all[i - 1].group !== h.group}
						<li class="group" role="presentation">{i18n.t(groupName[h.group])}</li>
					{/if}
					<li
						id="hit-{i}"
						role="option"
						aria-selected={i === sel}
						class:sel={i === sel}
						onmousemove={() => (sel = i)}
						onclick={() => go(h)}
					>
						<span class="ic">
							{#if h.img}<img src={h.img} alt="" />
							{:else if h.avatar}<Avatar name={h.avatar.name} src={h.avatar.src} size={30} />
							{:else if h.swatch}<i style:background={h.swatch}></i>
							{:else}→{/if}
						</span>
						<span class="text"
							><b>{h.label}</b>{#if h.sub}<small>{h.sub}</small>{/if}</span
						>
					</li>
				{/each}
				{#if q.trim().length >= 2 && !hits.length && !pages.length}
					<li class="none">{i18n.t('search.none', { q: q.trim() })}</li>
				{/if}
			</ul>
			<p class="keys">{i18n.t('search.keys')} · {session.username}</p>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 70;
		background: rgba(8, 10, 25, 0.5);
		display: flex;
		justify-content: center;
		align-items: flex-start;
		padding: calc(60px + env(safe-area-inset-top, 0px)) 16px 16px;
		animation: fade 0.15s;
	}
	.pal {
		width: min(560px, 100%);
		background: var(--surface);
		border-radius: 18px;
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
		overflow: hidden;
		animation: up 0.25s;
	}
	input {
		width: 100%;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: none;
		padding: 16px 18px;
		font-size: 16px;
		outline: none;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 6px;
		max-height: min(420px, 60dvh);
		overflow: auto;
	}
	.group {
		font: 600 10.5px var(--mono);
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--muted);
		padding: 10px 10px 4px;
	}
	[role='option'] {
		display: grid;
		grid-template-columns: 34px minmax(0, 1fr);
		gap: 10px;
		align-items: center;
		padding: 7px 10px;
		border-radius: 10px;
		cursor: pointer;
	}
	.sel {
		background: var(--accent-soft);
	}
	.ic {
		width: 34px;
		height: 34px;
		border-radius: 8px;
		overflow: hidden;
		background: var(--surface-2);
		display: grid;
		place-items: center;
		color: var(--muted);
	}
	.ic img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.ic i {
		width: 16px;
		height: 16px;
		border-radius: 50%;
	}
	.text {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	.text b {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	small {
		color: var(--muted);
		font-size: 12px;
	}
	.none {
		color: var(--muted);
		padding: 16px 10px;
		font-size: 13.5px;
	}
	.keys {
		border-top: 1px solid var(--line);
		padding: 8px 14px;
		font-size: 11.5px;
		color: var(--muted);
	}
</style>
