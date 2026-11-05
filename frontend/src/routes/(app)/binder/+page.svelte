<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog } from '#lib/session.svelte.ts';
	import { artUrl } from '#lib/api.ts';
	import { pad3, tone } from '#lib/fx.ts';
	import type { CardDto, OwnedDto } from '#lib/types.ts';
	import Card from '#lib/components/Card.svelte';
	import CardInspector from '#lib/components/CardInspector.svelte';
	import CollectionPicker from '#lib/components/CollectionPicker.svelte';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const catalog = getCatalog()!;

	type Filter = 'all' | 'owned' | 'missing' | 'dupes';
	const PER_PAGE = 9;

	// Local copy of the counts: selling updates it without a reload
	let owned = $state<Record<string, OwnedDto>>({});
	let completion = $state<Record<string, number>>({});
	$effect(() => {
		owned = Object.fromEntries(data.binder.cards.map((c) => [c.card, c]));
		completion = { ...data.binder.collections };
	});

	let filter = $state<Filter>('all');
	let page = $state(0);
	let spread = $state(false);
	let inspecting = $state<CardDto | null>(null);
	let slide = $state<'l' | 'r' | ''>('');

	onMount(() => {
		const mq = matchMedia('(min-width: 1000px)');
		const set = () => (spread = mq.matches);
		set();
		mq.addEventListener('change', set);
		return () => mq.removeEventListener('change', set);
	});

	const cards = $derived(
		(data.collection.cards ?? []).filter((c) => {
			const n = owned[c.slug]?.count ?? 0;
			return (
				filter === 'all' ||
				(filter === 'owned' && n > 0) ||
				(filter === 'missing' && n === 0) ||
				(filter === 'dupes' && n > 1)
			);
		})
	);
	const pages = $derived(Math.max(1, Math.ceil(cards.length / PER_PAGE)));
	const step = $derived(spread ? 2 : 1);
	const visible = $derived(
		Array.from({ length: step }, (_, k) => page + k)
			.filter((p) => p < pages)
			.map((p) => {
				const slice = cards.slice(p * PER_PAGE, (p + 1) * PER_PAGE);
				return [...slice, ...Array(PER_PAGE - slice.length).fill(null)] as (CardDto | null)[];
			})
	);
	const ownedHere = $derived(completion[data.collection.slug] ?? 0);

	// New set or filter: back to the first page
	$effect(() => {
		void data.collection.slug;
		void filter;
		page = 0;
	});
	// Spread pages start on even numbers
	$effect(() => {
		if (spread && page % 2) page -= 1;
	});

	function turn(d: number) {
		const next = Math.max(0, Math.min(pages - 1, page + d * step));
		if (next === page || (spread && next >= pages)) return;
		slide = d > 0 ? 'l' : 'r';
		page = next - (spread ? next % 2 : 0);
		tone(400, 0.04, 'sine', 0.03);
	}

	let x0: number | null = null;
	let swiped = false;
	const down = (e: PointerEvent) => {
		x0 = e.clientX;
		swiped = false;
	};
	const up = (e: PointerEvent) => {
		if (x0 === null) return;
		const dx = e.clientX - x0;
		x0 = null;
		if (Math.abs(dx) > 60) {
			swiped = true;
			turn(dx < 0 ? 1 : -1);
		}
	};
	const open = (c: CardDto) => {
		if (!swiped) inspecting = c;
	};

	let picking = $state(false);
	const setIndex = $derived(catalog.collections.findIndex((c) => c.slug === data.collection.slug));
	const showSet = (slug: string) => {
		picking = false;
		goto(`?set=${slug}`, { replace: true });
	};
	/** Previous / next set, wrapping around */
	function stepSet(d: number) {
		const n = catalog.collections.length;
		showSet(catalog.collections[(setIndex + d + n) % n].slug);
	}
	/** "/" opens the set picker, like a search box */
	function shortcut(e: KeyboardEvent) {
		const typing = (e.target as HTMLElement)?.closest('input, textarea, [contenteditable]');
		if (e.key === '/' && !typing && !picking && !inspecting) {
			e.preventDefault();
			picking = true;
		}
	}

	function sold(slug: string, left: { count: number; foils: number }) {
		if (left.count === 0) {
			delete owned[slug];
			completion[data.collection.slug] = Math.max(0, (completion[data.collection.slug] ?? 1) - 1);
		} else owned[slug] = { card: slug, ...left };
	}
</script>

<svelte:head><title>{i18n.t('nav.binder')} · AnyCardz</title></svelte:head>
<svelte:window onkeydown={shortcut} />

<div class="screen">
	<div class="head">
		<div class="set-nav">
			<button
				type="button"
				class="set-btn"
				aria-haspopup="dialog"
				onclick={() => (picking = true)}
				title={i18n.t('binder.pick_hint')}
				style:--cp={data.collection.primary}
			>
				<span class="ring big" style:--v={Math.round((ownedHere / data.collection.total) * 100)}
					><span>{Math.round((ownedHere / data.collection.total) * 100)}</span></span
				>
				<span class="set-text">
					<span class="eyebrow"
						>{i18n.t('binder.set_of', { i: setIndex + 1, n: catalog.collections.length })}</span
					>
					<span class="set-title"
						>{data.collection.title}<span class="chev" aria-hidden="true">▾</span></span
					>
				</span>
			</button>
			<div class="arrows">
				<button
					type="button"
					class="btn sm"
					onclick={() => stepSet(-1)}
					aria-label={i18n.t('binder.prev_set')}>‹</button
				>
				<button
					type="button"
					class="btn sm"
					onclick={() => stepSet(1)}
					aria-label={i18n.t('binder.next_set')}>›</button
				>
			</div>
		</div>
		<p class="stat">{i18n.t('binder.stat', { owned: ownedHere, total: data.collection.total })}</p>
	</div>

	<div class="seg" role="group" aria-label={i18n.t('binder.filter')}>
		{#each ['all', 'owned', 'missing', 'dupes'] as const as f}
			<button type="button" aria-pressed={filter === f} onclick={() => (filter = f)}
				>{i18n.t(`binder.${f}`)}</button
			>
		{/each}
	</div>

	{#if cards.length}
		<div class="binder" class:spread onpointerdown={down} onpointerup={up} role="presentation">
			{#each visible as pockets, vi (page + vi)}
				<div class="bpage" class:slide-l={slide === 'l'} class:slide-r={slide === 'r'}>
					{#each pockets as c, pi (c?.slug ?? `empty-${pi}`)}
						{#if !c}
							<div class="pocket"><span class="empty-pocket"></span></div>
						{:else}
							{@const o = owned[c.slug]}
							<button
								type="button"
								class="pocket"
								onclick={() => open(c)}
								aria-label={o
									? `${c.title}, ${i18n.t('inspect.copies', { n: o.count })}`
									: `#${c.number}, ${i18n.t('binder.not_found')}`}
							>
								{#if o}
									<Card card={c} foil={o.foils ? 'foil3' : null} tilt>
										{#snippet badge()}
											{#if o.count > 1}<span class="c-badge">×{o.count}</span>{/if}
											{#if o.foils}<span class="c-badge foilb">{i18n.t('card.foil')}</span>{/if}
										{/snippet}
									</Card>
								{:else}
									<div
										class="ghost r-tag-{c.rarity}"
										class:rare={c.rarity === 'rare'}
										class:legend={c.rarity === 'legendary'}
									>
										<img src={artUrl(c.slug)} alt="" loading="lazy" />
										<span class="g-r"></span><span class="g-no">#{pad3(c.number)}</span><span
											class="g-q">{i18n.t('binder.not_found')}</span
										>
									</div>
								{/if}
							</button>
						{/if}
					{/each}
				</div>
			{/each}
		</div>
		<div class="binder-nav">
			<button
				type="button"
				class="btn sm"
				onclick={() => turn(-1)}
				disabled={page === 0}
				aria-label={i18n.t('binder.prev')}>‹</button
			>
			<span class="mono">{i18n.t('binder.page', { i: page + 1, n: pages })}</span>
			<button
				type="button"
				class="btn sm"
				onclick={() => turn(1)}
				disabled={page + step >= pages}
				aria-label={i18n.t('binder.next')}>›</button
			>
		</div>
	{:else}
		<p class="emptynote">{i18n.t('binder.empty')}</p>
	{/if}
</div>

{#if picking}
	<CollectionPicker
		collections={catalog.collections}
		{completion}
		current={data.collection.slug}
		onPick={showSet}
		onClose={() => (picking = false)}
	/>
{/if}

{#if inspecting}
	{@const c = inspecting}
	<CardInspector
		card={c}
		owned={owned[c.slug]}
		onClose={() => (inspecting = null)}
		onSold={(left) => sold(c.slug, left)}
	/>
{/if}

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px max(16px, env(safe-area-inset-right, 0px)) 24px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 1200px;
		margin: 0 auto;
	}
	@media (max-width: 699px) {
		.screen {
			padding-bottom: 52px;
		}
	}
	@media (min-width: 700px) {
		.screen {
			padding: 24px;
		}
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: 12px;
		flex-wrap: wrap;
	}
	.stat {
		color: var(--muted);
		font: 600 13px var(--mono);
	}
	.seg {
		align-self: flex-start;
	}
	.set-nav {
		display: flex;
		align-items: center;
		gap: 10px;
		min-width: 0;
		flex: 1;
	}
	.set-btn {
		flex: 1 1 auto;
		display: flex;
		align-items: center;
		gap: 12px;
		min-width: 0;
		border: 1px solid transparent;
		background: none;
		padding: 6px 10px 6px 6px;
		margin-left: -6px;
		border-radius: 16px;
		text-align: left;
	}
	.set-btn:hover {
		background: var(--surface);
		border-color: var(--line);
	}
	.set-text {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
	}
	/* Inline, so a long title wraps with the chevron at its end */
	.set-title {
		font: 700 26px/1.05 var(--display);
		letter-spacing: -0.01em;
		min-width: 0;
		text-wrap: balance;
	}
	.chev {
		font-size: 15px;
		color: var(--muted);
		margin-left: 6px;
		white-space: nowrap;
	}
	/* Phones: the title gets the whole row, the count goes under it */
	@media (max-width: 699px) {
		.set-nav {
			flex-basis: 100%;
		}
	}
	.arrows {
		display: flex;
		gap: 6px;
		flex: none;
	}
	.ring {
		flex: none;
		width: 26px;
		height: 26px;
		border-radius: 50%;
		background: conic-gradient(var(--cp) calc(var(--v) * 1%), var(--surface-2) 0);
		display: grid;
		place-items: center;
		font: 700 8px var(--mono);
	}
	.ring::before {
		content: '';
		width: 18px;
		height: 18px;
		border-radius: 50%;
		background: var(--surface);
		grid-area: 1/1;
	}
	.ring.big {
		width: 46px;
		height: 46px;
		font-size: 11px;
	}
	.ring.big::before {
		width: 34px;
		height: 34px;
	}
	.ring span {
		grid-area: 1/1;
		z-index: 1;
	}
	.binder {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 34px;
		background: var(--surface-2);
		border-radius: 22px;
		padding: 14px;
		border: 1px solid var(--line);
		box-shadow: var(--shadow);
		touch-action: pan-y;
		max-width: 560px;
		width: 100%;
		margin: 0 auto;
	}
	.binder.spread {
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		max-width: none;
		padding: 22px;
	}
	/* Rings of the binder between the two pages */
	.binder.spread::before {
		content: '';
		position: absolute;
		left: 50%;
		top: 14px;
		bottom: 14px;
		width: 18px;
		margin-left: -9px;
		background:
			repeating-linear-gradient(transparent 0 26px, var(--line) 26px 30px, transparent 30px 40px),
			linear-gradient(90deg, transparent, rgba(0, 0, 0, 0.12), transparent);
		border-radius: 8px;
	}
	.bpage {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 12px;
	}
	.bpage.slide-l {
		animation: sl 0.35s ease-out;
	}
	.bpage.slide-r {
		animation: sr 0.35s ease-out;
	}
	@keyframes sl {
		from {
			transform: translateX(30%);
			opacity: 0;
		}
	}
	@keyframes sr {
		from {
			transform: translateX(-30%);
			opacity: 0;
		}
	}
	.pocket {
		position: relative;
		border: 0;
		padding: 0;
		background: none;
		border-radius: 8px;
		aspect-ratio: 63/88;
		display: block;
		width: 100%;
	}
	.pocket::after {
		content: '';
		position: absolute;
		inset: -4px;
		border-radius: 10px;
		background: linear-gradient(160deg, rgba(255, 255, 255, 0.35), transparent 40%);
		pointer-events: none;
		box-shadow: inset 0 0 0 1px var(--line);
	}
	.ghost {
		border-radius: 6.5%/4.7%;
		overflow: hidden;
		position: absolute;
		inset: 0;
		background: var(--surface);
		box-shadow: inset 0 0 0 1.5px var(--line);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 4px;
		text-align: center;
		padding: 8px;
	}
	.ghost.rare {
		box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--rare) 60%, transparent);
	}
	.ghost.legend {
		box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--legend) 60%, transparent);
	}
	.ghost img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		filter: grayscale(1) blur(7px) brightness(0.8);
		opacity: 0.22;
	}
	.g-no {
		font: 700 15px var(--mono);
		position: relative;
		color: var(--ink);
	}
	.g-q {
		font-size: 11px;
		color: var(--muted);
		position: relative;
	}
	.g-r {
		position: relative;
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--rc);
	}
	.empty-pocket {
		position: absolute;
		inset: 0;
		border-radius: 6.5%/4.7%;
		border: 1.5px dashed var(--line);
	}
	.binder-nav {
		display: flex;
		justify-content: center;
		align-items: center;
		gap: 14px;
		font-size: 12px;
		color: var(--muted);
	}
	.emptynote {
		color: var(--muted);
		font-size: 13px;
		padding: 16px;
		border: 1.5px dashed var(--line);
		border-radius: 12px;
		text-align: center;
	}
</style>
