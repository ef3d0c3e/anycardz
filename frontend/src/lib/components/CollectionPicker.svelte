<script lang="ts">
	import { onMount } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import type { CollectionDto } from '#lib/types.ts';
	import { RECENT_KEY } from '#lib/lobby/shelf.ts';

	interface Props {
		collections: CollectionDto[];
		/** Distinct cards owned per collection */
		completion: Record<string, number>;
		current: string;
		/** Adds an "all sets" entry at the top, picked as '' */
		allLabel?: string;
		allCount?: number;
		/** Shown on each row instead of collection progress, e.g. cards in a tray */
		counts?: Record<string, number>;
		onPick: (slug: string) => void;
		onClose: () => void;
	}
	let { collections, completion, current, allLabel, allCount, counts, onPick, onClose }: Props =
		$props();
	const i18n = getI18n();

	type Sort = 'progress' | 'name' | 'number';
	type Status = 'all' | 'started' | 'new' | 'complete';

	let query = $state('');
	let sort = $state<Sort>('progress');
	let status = $state<Status>('all');
	let recent = $state<string[]>([]);
	let input = $state<HTMLInputElement>();
	let list = $state<HTMLUListElement>();

	onMount(() => {
		try {
			recent = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
		} catch {
			recent = [];
		}
		input?.focus();
	});

	const owned = (c: CollectionDto) => completion[c.slug] ?? 0;
	const pct = (c: CollectionDto) => Math.round((owned(c) / c.total) * 100);
	// Accents and case do not matter: "fleur" finds "Fleurs", "elements" finds "Éléments"
	const fold = (s: string) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

	const shown = $derived.by(() => {
		const q = fold(query.trim());
		const list = collections.filter((c) => {
			const n = owned(c);
			const st =
				status === 'all' ||
				(status === 'started' && n > 0 && n < c.total) ||
				(status === 'new' && n === 0) ||
				(status === 'complete' && n === c.total);
			return st && (!q || fold(c.title).includes(q));
		});
		return list.toSorted((a, b) => {
			if (sort === 'name') return a.title.localeCompare(b.title, i18n.lang);
			if (sort === 'number') return a.number - b.number;
			// Progress: sets in progress first, most complete on top, untouched sets last
			return pct(b) - pct(a) || a.number - b.number;
		});
	});
	const recentSets = $derived(
		recent
			.map((s) => collections.find((c) => c.slug === s))
			.filter((c): c is CollectionDto => !!c && c.slug !== current)
	);

	function pick(slug: string) {
		if (!slug) return onPick(slug);
		try {
			localStorage.setItem(
				RECENT_KEY,
				JSON.stringify([slug, ...recent.filter((s) => s !== slug)].slice(0, 6))
			);
		} catch {
			/* not remembered */
		}
		onPick(slug);
	}

	/** Arrow keys move between the search box and the rows */
	function nav(e: KeyboardEvent) {
		const rows = [...(list?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
		const at = rows.indexOf(document.activeElement as HTMLButtonElement);
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			rows[Math.min(rows.length - 1, at + 1)]?.focus();
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			if (at <= 0) input?.focus();
			else rows[at - 1]?.focus();
		} else if (e.key === 'Enter' && document.activeElement === input && shown[0]) {
			e.preventDefault();
			pick(shown[0].slug);
		}
	}
	const parts = (title: string) => {
		const q = fold(query.trim());
		const i = q ? fold(title).indexOf(q) : -1;
		return i < 0
			? [title, '', '']
			: [title.slice(0, i), title.slice(i, i + q.length), title.slice(i + q.length)];
	};
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="modal" onclick={(e) => e.target === e.currentTarget && onClose()}>
	<div
		class="sheet"
		role="dialog"
		aria-modal="true"
		aria-labelledby="picker-title"
		tabindex="-1"
		onkeydown={nav}
	>
		<div class="top">
			<h2 id="picker-title">{i18n.t('picker.title')}</h2>
			<button type="button" class="x" onclick={onClose} aria-label={i18n.t('common.close')}
				>✕</button
			>
		</div>
		<input
			bind:this={input}
			bind:value={query}
			id="set-search"
			type="search"
			autocomplete="off"
			placeholder={i18n.t('picker.search', { n: collections.length })}
			aria-label={i18n.t('picker.search', { n: collections.length })}
			aria-controls="set-list"
		/>
		<div class="filters">
			<div class="seg" role="group" aria-label={i18n.t('picker.sort')}>
				{#each ['progress', 'name', 'number'] as const as s}
					<button type="button" aria-pressed={sort === s} onclick={() => (sort = s)}
						>{i18n.t(`picker.sort_${s}`)}</button
					>
				{/each}
			</div>
			<div class="seg" role="group" aria-label={i18n.t('picker.status')}>
				{#each ['all', 'started', 'new', 'complete'] as const as s}
					<button type="button" aria-pressed={status === s} onclick={() => (status = s)}
						>{i18n.t(`picker.status_${s}`)}</button
					>
				{/each}
			</div>
		</div>
		{#if recentSets.length && !query}
			<div class="recent">
				<span class="eyebrow">{i18n.t('picker.recent')}</span>
				{#each recentSets as c (c.slug)}
					<button type="button" class="chip" style:--cp={c.primary} onclick={() => pick(c.slug)}
						><i></i>{c.title}</button
					>
				{/each}
			</div>
		{/if}
		<p class="count" aria-live="polite">{i18n.t('picker.count', { n: shown.length })}</p>
		<ul class="list" id="set-list" bind:this={list}>
			{#if allLabel && !query}
				<li>
					<button
						type="button"
						class="row all"
						aria-current={current === '' ? 'true' : undefined}
						onclick={() => pick('')}
					>
						<span class="ring all-ring" aria-hidden="true">✱</span>
						<span class="name">{allLabel}</span>
						<span class="n mono"
							>{allCount !== undefined ? i18n.t('picker.cards', { n: allCount }) : ''}</span
						>
					</button>
				</li>
			{/if}
			{#each shown as c (c.slug)}
				{@const [a, m, b] = parts(c.title)}
				<li>
					<button
						type="button"
						class="row"
						aria-current={c.slug === current ? 'true' : undefined}
						style:--cp={c.primary}
						style:--cs={c.secondary}
						onclick={() => pick(c.slug)}
					>
						<span class="ring" style:--v={pct(c)}><span>{pct(c)}</span></span>
						<span class="name">{a}<mark>{m}</mark>{b}</span>
						<span class="n mono"
							>{counts
								? i18n.t('picker.cards', { n: counts[c.slug] ?? 0 })
								: `${owned(c)}/${c.total}`}</span
						>
						<span class="bar"><i style:width="{pct(c)}%"></i></span>
					</button>
				</li>
			{:else}
				<li class="none">{i18n.t('picker.none', { q: query })}</li>
			{/each}
		</ul>
	</div>
</div>

<style>
	.modal {
		position: fixed;
		inset: 0;
		z-index: 60;
		display: grid;
		place-items: end center;
		background: rgba(8, 10, 25, 0.6);
		backdrop-filter: blur(6px);
		animation: fade 0.2s;
	}
	@media (min-width: 640px) {
		.modal {
			place-items: start center;
			padding: 8vh 16px 16px;
		}
	}
	.sheet {
		width: min(720px, 100%);
		height: min(86dvh, 760px);
		display: flex;
		flex-direction: column;
		gap: 12px;
		background: var(--surface);
		border-radius: 24px 24px 0 0;
		padding: 18px max(16px, env(safe-area-inset-right, 0px)) 0
			max(16px, env(safe-area-inset-left, 0px));
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
		animation: up 0.3s cubic-bezier(0.2, 1.2, 0.4, 1);
		outline: none;
	}
	@media (min-width: 640px) {
		.sheet {
			border-radius: 24px;
			height: min(80dvh, 760px);
			padding: 22px 22px 0;
		}
	}
	.top {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	h2 {
		font-size: 24px;
	}
	.x {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		border: 0;
		background: var(--surface-2);
		font-size: 18px;
	}
	input {
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 12px;
		padding: 12px 14px;
		font-size: 16px;
		width: 100%;
	}
	.filters {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.filters .seg {
		flex-wrap: wrap;
	}
	.recent {
		display: flex;
		gap: 6px;
		align-items: center;
		overflow-x: auto;
		padding-bottom: 2px;
	}
	.recent .eyebrow {
		flex: none;
		margin-right: 4px;
	}
	.chip {
		flex: none;
		display: flex;
		align-items: center;
		gap: 6px;
		border: 1px solid var(--line);
		background: var(--surface);
		border-radius: 999px;
		padding: 5px 10px;
		font-size: 12.5px;
		font-weight: 600;
	}
	.chip i {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--cp);
	}
	.count {
		font: 600 11px var(--mono);
		color: var(--muted);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}
	.list {
		list-style: none;
		margin: 0 -6px;
		padding: 0 6px 16px;
		overflow: auto;
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		align-content: start;
		gap: 6px;
	}
	@media (min-width: 640px) {
		.list {
			grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		}
	}
	.row {
		width: 100%;
		display: grid;
		grid-template-columns: 34px minmax(0, 1fr) auto;
		grid-template-rows: auto auto;
		column-gap: 10px;
		row-gap: 6px;
		align-items: center;
		text-align: left;
		padding: 10px 12px;
		border-radius: 14px;
		border: 1px solid var(--line);
		background: var(--surface);
	}
	.row:hover,
	.row:focus-visible {
		background: var(--surface-2);
	}
	.all-ring {
		background: var(--surface-2);
		font-size: 14px;
		color: var(--muted);
	}
	.all-ring::before {
		display: none;
	}
	.row.all {
		grid-template-rows: auto;
	}
	.row[aria-current='true'] {
		border-color: var(--ink);
		box-shadow: inset 0 0 0 1px var(--ink);
	}
	.ring {
		grid-row: 1 / 3;
		width: 34px;
		height: 34px;
		border-radius: 50%;
		background: conic-gradient(var(--cp) calc(var(--v) * 1%), var(--surface-2) 0);
		display: grid;
		place-items: center;
		font: 700 9px var(--mono);
	}
	.ring::before {
		content: '';
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--surface);
		grid-area: 1/1;
	}
	.ring span {
		grid-area: 1/1;
		z-index: 1;
	}
	.name {
		font-weight: 700;
		font-size: 14px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	mark {
		background: color-mix(in srgb, var(--legend) 35%, transparent);
		color: inherit;
		border-radius: 3px;
	}
	.n {
		font-size: 12px;
		color: var(--muted);
	}
	.bar {
		grid-column: 2 / 4;
		height: 4px;
		border-radius: 2px;
		background: var(--surface-2);
		overflow: hidden;
	}
	.bar i {
		display: block;
		height: 100%;
		background: linear-gradient(90deg, var(--cp), var(--cs));
	}
	.none {
		color: var(--muted);
		padding: 24px 8px;
		text-align: center;
		grid-column: 1 / -1;
	}
</style>
