<script lang="ts">
	import { onDestroy, onMount, tick as nextTick } from 'svelte';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog, getSession, getToasts } from '#lib/session.svelte.ts';
	import { post } from '#lib/api.ts';
	import { arp, flyCoins, flyTo, tone } from '#lib/fx.ts';
	import type { FreePacksDto, StreakDto } from '#lib/types.ts';
	import Pack from '#lib/components/Pack.svelte';
	import HoldButton from '#lib/components/HoldButton.svelte';
	import ScratchTicket from '#lib/components/ScratchTicket.svelte';
	import Missions from '#lib/components/Missions.svelte';
	import { getTopup } from '#lib/topup.svelte.ts';
	import { ApiError } from '#lib/api.ts';
	import CollectionPicker from '#lib/components/CollectionPicker.svelte';
	import { readRecent, SHELF_MAX, shelfOrder } from '#lib/lobby/shelf.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const session = getSession();
	const catalog = getCatalog()!;
	const toasts = getToasts();
	/** Packs drawn in the pile; a badge counts the rest */
	const MAX_STACK = 3;
	const topup = getTopup();

	const pileNames = $derived(
		[...new Set(session.pile)].map((s) => catalog.get(s)?.title ?? s).join(' · ')
	);

	// ---- daily streak ----
	let ticket = $state<HTMLElement>();
	let claiming = false;
	async function claim() {
		if (claiming || session.streak.claimed_today) return;
		claiming = true;
		try {
			const res = await post<{ reward: number; coins: number; streak: StreakDto }>('/streak/claim');
			arp([660, 880, 1100, 1320]);
			if (ticket) await flyCoins(ticket);
			session.coins = res.coins;
			session.streak = res.streak;
			toasts.show(i18n.t('streak.claimed_toast', { n: res.reward, day: res.streak.day }));
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		} finally {
			claiming = false;
		}
	}
	let countdown = $state('');
	const tick = () => {
		const now = new Date();
		// The API counts days in UTC
		const next = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1);
		const s = Math.max(0, Math.floor((next - now.getTime()) / 1000));
		countdown = [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60]
			.map((v) => String(v).padStart(2, '0'))
			.join(':');
	};
	tick();
	const timer = setInterval(() => {
		tick();
		freeTick();
	}, 1000);
	onDestroy(() => clearInterval(timer));

	// ---- free packs, twice a day ----
	let freeLeft = $state('');
	let claimingFree = $state(false);
	let freeEl = $state<HTMLElement>();
	function freeTick() {
		const next = session.freePacks.next_at;
		if (!next) return;
		const s = Math.max(0, Math.floor((new Date(next).getTime() - Date.now()) / 1000));
		// Time is up: the button comes back without a reload
		if (s === 0) session.freePacks = { ...session.freePacks, ready: true, next_at: null };
		freeLeft = [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60]
			.map((v) => String(v).padStart(2, '0'))
			.join(':');
	}
	freeTick();
	async function claimFree() {
		if (claimingFree || !session.freePacks.ready) return;
		claimingFree = true;
		try {
			const res = await post<{ pile: string[]; free_packs: FreePacksDto }>('/free-packs/claim');
			session.freePacks = res.free_packs;
			// The pile now, not after the flight: news arriving meanwhile (a mission's
			// bonus pack) would be overwritten by this older answer
			session.pile = res.pile;
			freeTick();
			arp([520, 660, 780]);
			const target = document.querySelector('[data-nav="open"]');
			await nextTick();
			const loot = freeEl?.querySelector('.loot');
			if (target && loot) await flyTo(loot, target);
			toasts.show(i18n.t('free.added', { n: res.free_packs.count }));
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		} finally {
			claimingFree = false;
		}
	}

	// ---- shelf: a short list, the rest behind the set picker ----
	let recent = $state<string[]>([]);
	let browsing = $state(false);
	let flash = $state('');
	let shelfEl = $state<HTMLElement>();
	onMount(() => (recent = readRecent()));
	const shelf = $derived(shelfOrder(catalog.collections, data.binder.collections, recent));
	async function picked(slug: string) {
		browsing = false;
		// The picker remembered it: it comes first now
		recent = readRecent();
		flash = slug;
		await nextTick();
		shelfEl?.scrollIntoView({ block: 'start', behavior: 'smooth' });
		shelfEl?.scrollTo({ left: 0, behavior: 'smooth' });
		setTimeout(() => (flash = ''), 1600);
	}

	// ---- shop ----
	const packs = $state<Record<string, HTMLElement>>({});
	/** Not enough coins: the top-up sheet, which finishes the purchase afterwards */
	function askTopup(slug: string) {
		const c = catalog.get(slug);
		if (!c) return;
		topup.ask({
			need: c.price - session.coins,
			reason: i18n.t('topup.reason_booster', { name: c.title }),
			then: () => buy(slug)
		});
	}
	async function buy(slug: string) {
		const c = catalog.get(slug);
		if (c && session.coins < c.price) return askTopup(slug);
		try {
			const res = await post<{ coins: number; pile: string[] }>('/shop/boosters', {
				collection: slug
			});
			session.coins = res.coins;
			tone(520, 0.08, 'triangle', 0.05);
			const target = document.querySelector('[data-nav="open"]');
			if (target && packs[slug]) await flyTo(packs[slug], target);
			session.pile = res.pile;
			tone(780, 0.1, 'triangle', 0.05);
			toasts.show(i18n.t('lobby.added', { name: catalog.get(slug)?.title ?? slug }));
		} catch (e) {
			if (e instanceof ApiError && e.code === 'not_enough_coins') askTopup(slug);
			else toasts.show(i18n.error(e), 'warn');
		}
	}
</script>

<svelte:head><title>{i18n.t('nav.lobby')} · AnyCardz</title></svelte:head>

<div class="screen">
	<div class="lobby-grid">
		<div class="side">
			<section class="panel pile-hero" aria-labelledby="pile-title">
				{#if session.pile.length}
					<!-- Three packs at most: the room their spread needs is reserved beside them -->
					<a
						class="pile-stack"
						href="/open"
						aria-label={i18n.t('lobby.open_next')}
						style:--n={Math.min(session.pile.length, MAX_STACK)}
					>
						{#each session.pile.slice(0, MAX_STACK).toReversed() as slug, i}
							{@const c = catalog.get(slug)}
							{#if c}<Pack
									collection={c}
									style="--k:{Math.min(session.pile.length, MAX_STACK) - 1 - i}"
								/>{/if}
						{/each}
						{#if session.pile.length > MAX_STACK}
							<span class="more">+{session.pile.length - MAX_STACK}</span>
						{/if}
					</a>
				{:else}
					<div class="pile-empty">{i18n.t('lobby.empty_pile')}</div>
				{/if}
				<div class="pile-text">
					<p class="eyebrow">{i18n.t('lobby.unopened')}</p>
					<h2 id="pile-title">
						{session.pile.length
							? i18n.t('lobby.packs_waiting', { n: session.pile.length })
							: i18n.t('lobby.no_packs')}
					</h2>
					<p>{session.pile.length ? pileNames : i18n.t('lobby.no_packs_hint')}</p>
					{#if session.pile.length}<a class="btn" href="/open">{i18n.t('lobby.open_next')}</a>{/if}
				</div>
			</section>
			<Missions />
		</div>

		<div class="side">
			<section class="panel streak" aria-labelledby="streak-title">
				<div class="streak-head">
					<div>
						<p class="eyebrow">{i18n.t('streak.title')}</p>
						<h2 id="streak-title">
							{i18n.t('streak.day', { n: session.streak.day })}
							<span class="flame" aria-hidden="true"></span>
						</h2>
					</div>
					<p class="best">{i18n.t('streak.best', { n: session.streak.best })}</p>
				</div>
				<div bind:this={ticket}>
					{#if session.streak.claimed_today}
						<div class="claimed">
							<span class="t-label">{i18n.t('streak.today')}</span>
							<span class="t-val">+{i18n.num(session.streak.reward)}</span>
							<span class="t-stamp">{i18n.t('streak.claimed')}</span>
						</div>
					{:else}
						<ScratchTicket label={i18n.t('streak.scratch')} onScratched={claim}>
							<span class="t-label">{i18n.t('streak.today')}</span>
							<span class="t-val">+{i18n.num(session.streak.reward)}</span>
							<span class="t-sub">{i18n.t('common.coins')}</span>
						</ScratchTicket>
					{/if}
				</div>
				{#if session.streak.claimed_today}
					<p class="next">
						{i18n.t('streak.next', { time: countdown, n: session.streak.next_reward })}
					</p>
				{:else}
					<p class="next">
						{i18n.t('streak.scratch_hint')}
						<button type="button" class="linkish" onclick={claim}>{i18n.t('streak.reveal')}</button>
					</p>
				{/if}
				<ol class="ladder" aria-label={i18n.t('streak.ladder')}>
					{#each session.streak.rewards as v, i}
						{@const d = i + 1}
						{@const now = Math.min(session.streak.day, 5)}
						<li class:past={d < now} class:now={d === now}>
							<span>{i18n.t('streak.day', { n: d })}{d === 5 ? '+' : ''}</span><b>{i18n.num(v)}</b>
						</li>
					{/each}
				</ol>
			</section>

			<section class="panel free" aria-labelledby="free-title">
				<div class="streak-head">
					<div>
						<p class="eyebrow">{i18n.t('free.title')}</p>
						<h2 id="free-title">{i18n.t('free.heading', { n: session.freePacks.count })}</h2>
					</div>
					<p class="best">{i18n.t('free.every', { h: session.freePacks.every_hours })}</p>
				</div>
				<div bind:this={freeEl}>
					{#if session.freePacks.ready}
						<ScratchTicket label={i18n.t('streak.scratch')} onScratched={claimFree}>
							{@render freeLoot()}
						</ScratchTicket>
					{:else}
						<div class="claimed">
							{@render freeLoot()}
							<span class="t-stamp">{i18n.t('streak.claimed')}</span>
						</div>
					{/if}
				</div>
				<p class="next">
					{#if session.freePacks.ready}
						{i18n.t('free.scratch_hint')}
						<button type="button" class="linkish" onclick={claimFree}
							>{i18n.t('free.reveal')}</button
						>
					{:else}
						{i18n.t('free.next', { time: freeLeft })}
					{/if}
				</p>
			</section>
		</div>
	</div>

	{#snippet freeLoot()}
		<span class="loot" aria-hidden="true">
			{#if catalog.free}
				{@const free = catalog.free}
				{#each { length: session.freePacks.count } as _, i}
					<Pack collection={free} style="--i:{i}" />
				{/each}
			{/if}
		</span>
		<span class="t-val">+{session.freePacks.count}</span>
		<span class="t-sub">{i18n.t('free.packs')}</span>
	{/snippet}

	<div class="sec-head">
		<div>
			<h2>{i18n.t('lobby.boosters')}</h2>
			<p>{i18n.t('lobby.odds')}</p>
		</div>
		{#if catalog.collections.length > SHELF_MAX}
			<button type="button" class="btn sm" aria-haspopup="dialog" onclick={() => (browsing = true)}
				>{i18n.t('lobby.browse', { n: catalog.collections.length })}</button
			>
		{/if}
	</div>
	<div class="shelf" bind:this={shelfEl}>
		{#each shelf as c (c.slug)}
			{@const owned = data.binder.collections[c.slug] ?? 0}
			{@const short = session.coins < c.price}
			<article class="shelf-item" class:flash={flash === c.slug}>
				<div class="pack-wrap" bind:this={packs[c.slug]}><Pack collection={c} /></div>
				<h3 title={c.title}>{c.title}</h3>
				<div class="own">
					<span class="bar"><i style:width="{Math.round((owned / c.total) * 100)}%"></i></span
					>{owned}/{c.total}
				</div>
				<HoldButton
					class="btn buy {short ? 'short' : ''}"
					label={i18n.t(short ? 'lobby.topup_label' : 'lobby.buy_label', {
						name: c.title,
						price: c.price
					})}
					ms={short ? 350 : 650}
					onDone={() => buy(c.slug)}
					onProgress={(p) =>
						Math.round(p * 100) % 20 === 0 && tone(300 + p * 500, 0.03, 'sine', 0.03)}
					onTooShort={() => (short ? askTopup(c.slug) : toasts.show(i18n.t('lobby.keep_holding')))}
				>
					<span class="price"><span class="coin">c</span>{c.price}</span>
					<span class="verb">{short ? i18n.t('lobby.topup_short') : i18n.t('lobby.hold')}</span>
				</HoldButton>
			</article>
		{/each}
	</div>
</div>

{#if browsing}
	<CollectionPicker
		collections={catalog.collections}
		completion={data.binder.collections}
		current=""
		onPick={picked}
		onClose={() => (browsing = false)}
	/>
{/if}

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 18px;
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
	.lobby-grid {
		display: grid;
		gap: 16px;
		grid-template-columns: minmax(0, 1fr);
	}
	@media (min-width: 980px) {
		.lobby-grid {
			grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		}
	}
	.panel {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 20px;
		padding: 18px;
		min-width: 0;
	}
	.pile-hero {
		display: flex;
		flex-wrap: wrap;
		gap: 22px;
		align-items: center;
		background: radial-gradient(ellipse at 20% 50%, var(--felt-hi), var(--felt) 75%);
		color: var(--felt-ink);
		border: 0;
		overflow: hidden;
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-width: 0;
	}
	.free {
		display: flex;
		flex-direction: column;
		gap: 12px;
		/* The scratch ticket in the colours of the free packs */
		--legend: #b05cff;
	}
	.free h2 {
		font-size: 26px;
	}
	.loot {
		display: flex;
		width: 82px;
		margin-right: 4px;
	}
	.loot :global(.pack) {
		width: 42px;
		margin-right: -22px;
		rotate: calc((var(--i) - 1) * 9deg);
	}
	.loot :global(.pack:nth-child(2)) {
		translate: 0 -4px;
	}
	/* The content steps aside for the stamp */
	.free .claimed {
		padding-right: 120px;
	}
	.claimed .loot {
		opacity: 0.45;
	}
	.pile-hero .eyebrow {
		color: rgba(238, 240, 255, 0.6);
	}
	.pile-stack {
		position: relative;
		width: min(132px, 34vw);
		aspect-ratio: 5/8;
		flex: none;
		display: block;
		/* Room for the spread of the packs behind, hover fan included */
		margin: 8px calc(10px + (var(--n) - 1) * 22px) 8px 4px;
	}
	.pile-stack :global(.pack) {
		position: absolute;
		inset: 0;
		transform: rotate(calc((var(--k) * 5deg) - 2deg))
			translate(calc(var(--k) * 6px), calc(var(--k) * 3px));
		transition: transform 0.35s cubic-bezier(0.2, 1.4, 0.4, 1);
	}
	.pile-stack:hover :global(.pack),
	.pile-stack:focus-visible :global(.pack) {
		transform: rotate(calc((var(--k) * 8deg) - 4deg))
			translate(calc(var(--k) * 10px), calc(var(--k) * -4px));
	}
	.more {
		position: absolute;
		top: -8px;
		left: -6px;
		z-index: 2;
		min-width: 30px;
		height: 30px;
		padding: 0 8px;
		border-radius: 15px;
		display: grid;
		place-items: center;
		background: var(--felt-ink);
		color: var(--felt);
		font: 800 14px var(--mono);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
	}
	.pile-empty {
		display: grid;
		place-items: center;
		width: min(132px, 34vw);
		aspect-ratio: 5/8;
		border: 2px dashed rgba(238, 240, 255, 0.3);
		border-radius: 12px;
		color: rgba(238, 240, 255, 0.6);
		font-size: 12px;
		text-align: center;
		padding: 8px;
		flex: none;
	}
	.pile-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: flex-start;
		min-width: 0;
	}
	.pile-text h2 {
		font-size: 30px;
	}
	.pile-text p {
		color: rgba(238, 240, 255, 0.75);
	}
	/* Phones: a smaller stack, so the text keeps a usable width */
	@media (max-width: 599px) {
		.pile-hero {
			gap: 14px;
		}
		.pile-stack,
		.pile-empty {
			width: min(104px, 27vw);
		}
		.pile-stack {
			margin-right: calc(6px + (var(--n) - 1) * 18px);
		}
		.pile-text h2 {
			font-size: 24px;
		}
	}
	.pile-text .btn {
		white-space: normal;
		text-align: center;
		margin-top: 6px;
		background: var(--felt-ink);
		color: var(--felt);
		border-color: transparent;
	}
	.streak {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.streak-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: 10px;
	}
	.streak-head h2 {
		font-size: 26px;
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.flame {
		width: 18px;
		height: 24px;
		background: radial-gradient(ellipse at 50% 80%, #ffd25a, #ff7a1a 55%, #e8381b);
		border-radius: 50% 50% 45% 45% / 65% 65% 35% 35%;
		display: inline-block;
		animation: flick 1.2s ease-in-out infinite alternate;
		transform-origin: 50% 100%;
	}
	@keyframes flick {
		to {
			transform: scale(1.08, 0.92) rotate(3deg);
		}
	}
	.best,
	.next {
		color: var(--muted);
		font-size: 13px;
	}
	.t-label {
		font: 600 11px var(--mono);
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--muted);
	}
	.t-val {
		font: 800 42px/1 var(--display);
		color: var(--legend);
		letter-spacing: -0.03em;
	}
	.t-sub {
		font-weight: 600;
		color: var(--muted);
	}
	.claimed {
		position: relative;
		height: 96px;
		border-radius: 14px;
		background: var(--surface-2);
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
	}
	.t-stamp {
		position: absolute;
		right: 16px;
		top: 50%;
		translate: 0 -50%;
		rotate: -12deg;
		font: 800 15px/1 var(--display);
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--ok);
		border: 3px solid var(--ok);
		padding: 7px 10px;
		border-radius: 8px;
		animation: stamp 0.4s cubic-bezier(0.3, 1.6, 0.5, 1);
	}
	.ladder {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(5, 1fr);
		gap: 6px;
	}
	.ladder li {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		padding: 7px 2px;
		border-radius: 10px;
		background: var(--surface-2);
		font-size: 11px;
		color: var(--muted);
		position: relative;
		text-align: center;
	}
	.ladder li b {
		font: 700 13px var(--mono);
		color: var(--ink);
	}
	.ladder li.past {
		opacity: 0.55;
	}
	.ladder li.now {
		background: color-mix(in srgb, var(--legend) 18%, var(--surface));
		color: var(--ink);
		box-shadow: inset 0 0 0 1.5px var(--legend);
	}
	.sec-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
	}
	.sec-head h2 {
		font-size: 22px;
	}
	.sec-head p {
		color: var(--muted);
		font-size: 13px;
	}
	.shelf {
		display: flex;
		gap: 14px;
		overflow-x: auto;
		scroll-snap-type: x mandatory;
		padding: 4px 2px 12px;
	}
	@media (min-width: 700px) {
		.shelf {
			display: grid;
			grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
			overflow: visible;
		}
	}
	.shelf-item {
		flex: none;
		width: 168px;
		scroll-snap-align: start;
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}
	@media (min-width: 700px) {
		.shelf-item {
			width: auto;
		}
	}
	.pack-wrap {
		width: 78%;
		margin: 0 auto 4px;
		transition: transform 0.3s cubic-bezier(0.2, 1.4, 0.4, 1);
	}
	.shelf-item:hover .pack-wrap {
		transform: translateY(-6px) rotate(-2deg);
	}
	/* Two lines whatever the name: rows of buttons stay aligned */
	.shelf-item.flash .pack-wrap {
		animation: picked 0.8s ease-in-out 2;
	}
	@keyframes picked {
		50% {
			transform: translateY(-10px) scale(1.04);
			filter: drop-shadow(0 0 18px var(--accent));
		}
	}
	.shelf-item h3 {
		font-size: 15px;
		line-height: 1.15;
		min-height: 2.3em;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.own {
		display: flex;
		align-items: center;
		gap: 8px;
		font: 600 11.5px var(--mono);
		color: var(--muted);
	}
	.bar {
		flex: 1;
		height: 5px;
		border-radius: 3px;
		background: var(--surface-2);
		overflow: hidden;
		box-shadow: inset 0 0 0 1px var(--line);
	}
	.bar i {
		display: block;
		height: 100%;
		background: var(--ok);
		border-radius: 3px;
	}
	.verb {
		font-weight: 600;
		opacity: 0.8;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.shelf-item :global(.buy) {
		margin-top: auto;
		width: 100%;
		padding: 9px 10px;
		font-size: 13px;
		justify-content: space-between;
	}
	.price {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-family: var(--mono);
	}
	.shelf-item :global(.buy.short) {
		--fill: var(--legend);
		border-style: dashed;
	}
	.price .coin {
		width: 15px;
		height: 15px;
		font-size: 8px;
	}
</style>
