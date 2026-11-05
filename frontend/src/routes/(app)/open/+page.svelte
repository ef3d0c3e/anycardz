<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog, getSession, getToasts } from '#lib/session.svelte.ts';
	import { api, artUrl, post } from '#lib/api.ts';
	import { arp, burst, reducedMotion, sleep, tone } from '#lib/fx.ts';
	import { tilt } from '#lib/actions/tilt.ts';
	import {
		FREE_PACK,
		type CardDto,
		type CollectionDto,
		type OpenedDto,
		type Rarity
	} from '#lib/types.ts';
	import Card from '#lib/components/Card.svelte';
	import Pack from '#lib/components/Pack.svelte';
	import { getLive } from '#lib/live.svelte.ts';
	import { brag } from '#lib/brag.ts';
	import { playPack, ReelRun, type Mode } from '#lib/opening/reels.ts';

	const i18n = getI18n();
	const session = getSession();
	const catalog = getCatalog()!;
	const toasts = getToasts();
	const live = getLive();

	type Phase = 'empty' | 'sealed' | 'busy' | 'spinning' | 'landed' | 'done';
	interface Pulled {
		card: CardDto;
		foil: string | null;
		isNew: boolean;
		copies: number;
	}
	interface Reel {
		i: number;
		strip: { card: CardDto; foil: string | null }[];
		tell: Rarity | null;
		landed: boolean;
	}

	const RUN = 14; // cards the strip travels past the payline
	const PEEK = 0.16; // share of a card shown above and below the payline
	const GAP = 10;
	const RANK: Record<Rarity, number> = { common: 0, rare: 1, legendary: 2 };
	const SELL: Record<Rarity, number> = { common: 10, rare: 50, legendary: 250 };
	const MODE_KEY = 'anycardz-reel-mode';

	let phase = $state<Phase>('busy');
	let collection = $state<CollectionDto | null>(null);
	let pulled = $state<Pulled[]>([]);
	let reels = $state<Reel[]>([]);
	let tray = $state<(Pulled | null)[]>([]);
	let mode = $state<Mode>('one');
	let userMode = $state<Mode | null>(null);
	let geo = $state({ rw: 200, rh: 279, cell: 289, peek: 45 });
	let tear = $state(0);
	let torn = $state(false);
	let tearStarted = $state(false);
	let dim = $state(false);
	let rays = $state<'' | 'rare' | 'legendary'>('');
	let hint = $state('');
	let felt: HTMLDivElement;
	let arena = $state<HTMLDivElement>();
	const reelEls = $state<HTMLElement[]>([]);

	/** Taps and skips of the pack being opened, new for every pack */
	let run: ReelRun | null = null;
	let fillers: CardDto[] = [];

	const best = $derived(
		pulled.length
			? [...pulled].sort(
					(a, b) => RANK[b.card.rarity] - RANK[a.card.rarity] || Number(!!b.foil) - Number(!!a.foil)
				)[0]
			: null
	);
	const stats = $derived({
		news: pulled.filter((p) => p.isNew).length,
		dupes: pulled.filter((p) => !p.isNew).length,
		value: pulled.reduce((s, p) => s + SELL[p.card.rarity] * (p.foil ? 2 : 1), 0)
	});

	onMount(() => {
		try {
			const m = localStorage.getItem(MODE_KEY);
			if (m === 'one' || m === 'all') userMode = m;
		} catch {
			/* no remembered mode */
		}
		setup();
	});

	const pickMode = (): Mode => userMode ?? (felt.clientWidth >= 940 ? 'all' : 'one');

	/** One big reel on narrow screens, five side by side on wide ones, all fitting the height */
	function size(m: Mode) {
		const W = felt.clientWidth - 32;
		const H = felt.clientHeight;
		const byH = (H - 250) / (1 + 2 * PEEK) / (88 / 63);
		const rw = Math.max(
			70,
			Math.floor(m === 'all' ? Math.min(210, (W - 4 * 14) / 5, byH) : Math.min(260, W * 0.64, byH))
		);
		const rh = (rw * 88) / 63;
		geo = { rw, rh, cell: rh + GAP, peek: Math.round(rh * PEEK) };
	}

	function setup() {
		pulled = [];
		reels = [];
		torn = false;
		tear = 0;
		tearStarted = false;
		dim = false;
		rays = '';
		collection = null;
		const slug = session.pile[0];
		if (!slug) {
			phase = 'empty';
			return;
		}
		collection = catalog.get(slug) ?? null;
		mode = pickMode();
		size('one');
		tray = Array(5).fill(null);
		phase = 'sealed';
		hint = i18n.t('open.tear_hint');
		// Reel fillers: cards of the set, fetched while the player tears
		fillers = [];
		fetchFillers(slug)
			.then((cards) => (fillers = cards))
			.catch(() => {});
	}

	/** Cards that spin past on the reels. A free pack draws from everything:
	 * a few collections picked at random stand for the whole catalog. */
	async function fetchFillers(slug: string): Promise<CardDto[]> {
		if (slug !== FREE_PACK) return (await api<CollectionDto>(`/collections/${slug}`)).cards ?? [];
		const picks = [...catalog.collections].sort(() => Math.random() - 0.5).slice(0, 3);
		const sets = await Promise.all(picks.map((c) => api<CollectionDto>(`/collections/${c.slug}`)));
		return sets.flatMap((c) => c.cards ?? []);
	}

	// ---- tear strip ----
	let minX: number | null = null;
	let maxX = 0;
	let lastStep = 0;
	function tearDown(e: PointerEvent) {
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
		minX = maxX = e.clientX;
		tearStarted = true;
	}
	function tearMove(e: PointerEvent) {
		if (minX === null || phase !== 'sealed') return;
		minX = Math.min(minX, e.clientX);
		maxX = Math.max(maxX, e.clientX);
		const width = (e.currentTarget as HTMLElement).clientWidth * 0.78;
		tear = Math.min(1, Math.max(tear, (maxX - minX) / width));
		const s = Math.floor(tear * 12);
		if (s > lastStep) {
			lastStep = s;
			tone(1800 + Math.random() * 1500, 0.012, 'sawtooth', 0.02);
		}
		if (tear >= 0.9) tearDone();
	}

	async function tearDone() {
		if (phase !== 'sealed' || !collection) return;
		phase = 'busy';
		tear = 1;
		torn = true;
		lastStep = 0;
		tone(200, 0.25, 'sawtooth', 0.04);
		tone(120, 0.3, 'square', 0.02, 0.05);
		const slug = collection.slug;
		let opened: OpenedDto;
		let full: CollectionDto;
		try {
			[opened, full] = await Promise.all([
				post<OpenedDto>('/pile/open', { collection: slug }),
				fillers.length
					? Promise.resolve({ ...collection, cards: fillers })
					: fetchFillers(slug).then((cards) => ({ ...collection!, cards })),
				sleep(700)
			]).then(([o, c]) => [o, c] as const);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
			setup();
			return;
		}
		session.pile = opened.pile;
		session.pityLeft = opened.pity_left;
		fillers = full.cards ?? [];
		const bySlug = new Map(fillers.map((c) => [c.slug, c]));
		// From a free pack, the won cards may come from collections not fetched
		const missing = opened.cards.map((p) => p.card).filter((s) => !bySlug.has(s));
		if (missing.length) {
			const found = await api<Record<string, CardDto>>(
				`/cards?slugs=${missing.map(encodeURIComponent).join(',')}`
			).catch(() => ({}) as Record<string, CardDto>);
			for (const c of Object.values(found)) bySlug.set(c.slug, c);
		}
		pulled = opened.cards.flatMap((p) => {
			const card = bySlug.get(p.card);
			return card ? [{ card, foil: p.foil, isNew: p.is_new, copies: p.copies }] : [];
		});
		// The won cards' artwork, so it is there when the reel stops
		for (const p of pulled) new Image().src = artUrl(p.card.slug, 840);
		mode = pickMode();
		size(mode);
		await runReels();
	}

	// ---- reels ----
	function randomFiller(): CardDto {
		const n = Math.random() * 100;
		const r: Rarity = n < 80 ? 'common' : n < 95 ? 'rare' : 'legendary';
		const pool = fillers.filter((c) => c.rarity === r);
		const from = pool.length ? pool : fillers;
		return from[Math.floor(Math.random() * from.length)];
	}
	function buildReel(i: number): Reel {
		const p = pulled[i];
		const strip = Array.from({ length: RUN + 3 }, (_, j) =>
			j === 1
				? { card: p.card, foil: p.foil }
				: { card: randomFiller(), foil: Math.random() < 0.05 ? 'foil3' : null }
		);
		return { i, strip, tell: null, landed: false };
	}

	/** Drops the strip past the payline (quadratic ease-out), overshoots, springs back onto the won card */
	function spin(index: number, dur: number, r: ReelRun): Promise<void> {
		return new Promise((done) => {
			const el = reelEls[index];
			const strip = el.querySelector<HTMLElement>('.rstrip')!;
			const reel = reels[index];
			const rarity = pulled[reel.i].card.rarity;
			const { cell, peek } = geo;
			const start = peek - (1 + RUN) * cell;
			const end = peek - cell;
			const over = 0.3 * cell;
			const total = end + over - start;
			strip.style.transform = `translateY(${start}px)`;
			const t0 = performance.now();
			let passed = 0;
			let told = false;
			const frame = (now: number) => {
				const t = r.stopping || reducedMotion() ? 1 : Math.min(1, (now - t0) / dur);
				const y = start + total * (1 - (1 - t) * (1 - t));
				strip.style.transform = `translateY(${y}px)`;
				const p = Math.floor((y - start) / cell);
				if (p > passed) {
					passed = p;
					if (t < 1) tone(900 + Math.random() * 200, 0.012, 'square', 0.018);
				}
				// The tell: the frame glows as the reel slows onto a rare or better
				if (!told && rarity !== 'common' && (end - y) / cell < 2.4) {
					told = true;
					reel.tell = rarity;
					if (rarity === 'legendary') dim = true;
					if (t < 1) tone(rarity === 'legendary' ? 220 : 330, 0.35, 'sine', 0.04);
				}
				if (t < 1) {
					requestAnimationFrame(frame);
					return;
				}
				strip.style.transform = `translateY(${end}px)`;
				strip
					.animate(
						[{ transform: `translateY(${end + over}px)` }, { transform: `translateY(${end}px)` }],
						{
							duration: r.skippingAll || reducedMotion() ? 1 : 300,
							easing: 'cubic-bezier(.34,1.3,.64,1)'
						}
					)
					.finished.then(() => done());
			};
			requestAnimationFrame(frame);
		});
	}

	function land(index: number) {
		const reel = reels[index];
		const p = pulled[reel.i];
		reel.landed = true;
		tone(500, 0.05, 'triangle', 0.05);
		const el = reelEls[index];
		if (p.card.rarity === 'legendary') {
			rays = 'legendary';
			burst(el, '#ffcf5c', 40);
			arp([523, 659, 784, 1046, 1318], 0.09, 'triangle', 0.06);
		} else if (p.card.rarity === 'rare') {
			if (!rays) rays = 'rare';
			burst(el, '#41e4cf', 18);
			arp([660, 880, 1175], 0.08);
		}
		if (p.foil) setTimeout(() => arp([1318, 1568, 1976], 0.06, 'sine', 0.04), 200);
	}

	async function runReels() {
		const r = new ReelRun();
		run = r;
		phase = 'spinning';
		const cards = pulled.map((p) => ({ rarity: p.card.rarity, foil: p.foil }));
		await playPack(cards, mode, r, {
			show: async (indexes) => {
				if (mode === 'one') {
					dim = false;
					rays = '';
				}
				reels = indexes.map((i) => buildReel(i));
				await tick();
			},
			spin: (slot, ms) => spin(slot, ms, r),
			land: (slot) => land(slot),
			tray: (i) => (tray[i] = pulled[i]),
			status: (p, i) => {
				phase = p;
				const c = pulled[i].card;
				if (mode === 'all') hint = i18n.t('open.tap_stop');
				else if (p === 'spinning') hint = i18n.t('open.card_n', { i: i + 1, n: pulled.length });
				else
					hint =
						c.rarity === 'legendary'
							? i18n.t('open.legendary', { name: c.title })
							: i18n.t(i < pulled.length - 1 ? 'open.tap_continue' : 'open.tap_finish');
			}
		});
		if (mode === 'all') await sleep(500);
		dim = false;
		phase = 'done';
		hint = '';
		if (mode === 'one') rays = '';
	}

	function skip() {
		if (phase === 'spinning' || phase === 'landed') run?.skip();
	}
	function nudge() {
		if (phase === 'spinning' || phase === 'landed') run?.tap();
	}
	function key(e: KeyboardEvent) {
		if (e.key !== ' ' && e.key !== 'Enter') return;
		e.preventDefault();
		if (phase === 'sealed') tearDone();
		else nudge();
	}
	function chooseMode(m: Mode) {
		userMode = m;
		try {
			localStorage.setItem(MODE_KEY, m);
		} catch {
			/* not remembered */
		}
		if (phase === 'spinning' || phase === 'landed') toasts.show(i18n.t('open.next_mode'));
	}
</script>

<svelte:head><title>{i18n.t('nav.open')} · AnyCardz</title></svelte:head>

<div
	class="felt"
	class:dim
	class:rays={!!rays}
	class:rare-rays={rays === 'rare'}
	bind:this={felt}
	style:--rw="{geo.rw}px"
	style:--rh="{geo.rh}px"
	style:--peek="{geo.peek}px"
	style:--gap="{GAP}px"
>
	<div class="open-head">
		<div>
			{#if collection}
				<p class="eyebrow">
					{i18n.t('open.pile_count', { n: session.pile.length + (phase === 'sealed' ? 0 : 1) })}
				</p>
				<h1>{collection.title}</h1>
				<p class="pity" title={i18n.t('pity.hint')}>
					{i18n.t('pity.left', { n: session.pityLeft })}
				</p>
			{/if}
		</div>
		<div class="open-ctrls">
			<div class="seg" role="group" aria-label={i18n.t('open.mode')}>
				<button
					type="button"
					aria-pressed={(userMode ?? mode) === 'one'}
					onclick={() => chooseMode('one')}>{i18n.t('open.one_by_one')}</button
				>
				<button
					type="button"
					aria-pressed={(userMode ?? mode) === 'all'}
					onclick={() => chooseMode('all')}>{i18n.t('open.all_at_once')}</button
				>
			</div>
			{#if phase === 'spinning' || phase === 'landed'}
				<button type="button" class="ghost-btn" onclick={skip}>{i18n.t('open.skip')}</button>
			{/if}
		</div>
	</div>

	{#if phase === 'empty'}
		<div class="empty">
			<h2>{i18n.t('open.empty_title')}</h2>
			<p>{i18n.t('open.empty_hint')}</p>
			<a class="btn" href="/">{i18n.t('open.go_lobby')}</a>
		</div>
	{:else}
		<div
			class="arena"
			bind:this={arena}
			tabindex="0"
			role="button"
			aria-label={i18n.t('open.arena')}
			onpointerdown={nudge}
			onkeydown={key}
		>
			{#if collection && (phase === 'sealed' || (phase === 'busy' && !reels.length))}
				<div class="tear-wrap tilt" use:tilt>
					<div class="tear-pack" class:torn class:started={tearStarted} style:--t={tear}>
						<div class="tp-body"><Pack {collection} /></div>
						<div class="tp-flap"><Pack {collection} /></div>
						<div
							class="tear-zone"
							role="slider"
							tabindex="-1"
							aria-label={i18n.t('open.tear_label')}
							aria-valuemin={0}
							aria-valuemax={100}
							aria-valuenow={Math.round(tear * 100)}
							onpointerdown={(e) => {
								e.stopPropagation();
								tearDown(e);
							}}
							onpointermove={tearMove}
							onpointerup={() => (minX = null)}
							onpointercancel={() => (minX = null)}
						>
							<span class="tear-guide"></span><span class="tear-line"></span><span class="tear-hand"
							></span>
						</div>
					</div>
				</div>
			{:else if phase === 'done' && mode === 'one' && best}
				<div class="best"><Card card={best.card} foil={best.foil} tilt large /></div>
			{:else if reels.length}
				<div class="reels" class:all={mode === 'all'}>
					{#each reels as reel, ri (reel.i)}
						<div
							class="reel"
							class:landed={reel.landed}
							class:tell-rare={reel.tell === 'rare'}
							class:tell-legendary={reel.tell === 'legendary'}
							bind:this={reelEls[ri]}
						>
							<div class="reel-win">
								<div class="rstrip">
									{#each reel.strip as cell, j}
										<div class="cell" class:win={j === 1}>
											<Card
												card={cell.card}
												foil={cell.foil}
												eager
												large={mode === 'one' && j === 1}
											/>
										</div>
									{/each}
								</div>
							</div>
							<!-- Over the payline, outside the clipped window: the rarity frame,
							     then the landed card, free to tilt and show its badge -->
							<div class="payline" aria-hidden={!reel.landed}>
								{#if reel.landed}
									{@const p = pulled[reel.i]}
									<div class="landed-card">
										<Card card={p.card} foil={p.foil} tilt large={mode === 'one'}>
											{#snippet badge()}
												{#if p.isNew}<span class="c-badge new">{i18n.t('open.new')}</span>
												{:else}<span class="c-badge">×{p.copies}</span>{/if}
											{/snippet}
										</Card>
									</div>
								{/if}
							</div>
							<span class="pay l"></span><span class="pay r"></span>
						</div>
					{/each}
				</div>
			{/if}
		</div>
		<p class="hint" aria-live="polite">
			{hint}{#if phase === 'sealed'}<span class="sr"> {i18n.t('open.tear_sr')}</span>{/if}
		</p>

		{#if phase === 'done'}
			<div class="after">
				<div class="stats">
					<div><b>{stats.news}</b><span>{i18n.t('open.stat_new')}</span></div>
					<div><b>{stats.dupes}</b><span>{i18n.t('open.stat_dupes')}</span></div>
					<div>
						<b class="mono">{i18n.num(stats.value)}</b><span>{i18n.t('open.stat_value')}</span>
					</div>
				</div>
				<div class="row">
					{#if session.pile.length}
						<button type="button" class="btn primary" onclick={setup}
							>{i18n.t('open.next', { n: session.pile.length })}</button
						>
					{:else}
						<a class="btn primary" href="/">{i18n.t('open.buy_more')}</a>
					{/if}
					<a class="btn" href={collection?.free ? '/binder' : `/binder?set=${collection?.slug}`}
						>{i18n.t('open.see_binder')}</a
					>
					{#if best}
						<button
							type="button"
							class="btn"
							onclick={() => brag(best.card.slug, !!best.foil, { i18n, live, toasts })}
							>{i18n.t('live.brag', { name: best.card.title })}</button
						>
					{/if}
				</div>
			</div>
		{/if}

		<div class="tray">
			{#each tray as slot}
				{#if slot}
					<div class="slotmini pop">
						<Card card={slot.card} foil={slot.foil}>
							{#snippet badge()}{#if slot.isNew}<span class="c-badge new">{i18n.t('open.new')}</span
									>{/if}{/snippet}
						</Card>
					</div>
				{:else}
					<span class="slotmini empty"></span>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.pity {
		font-size: 12.5px;
		color: rgba(238, 240, 255, 0.62);
		margin-top: 2px;
	}
	.felt {
		position: relative;
		min-height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 14px;
		padding: 16px max(16px, env(safe-area-inset-right, 0px)) 20px
			max(16px, env(safe-area-inset-left, 0px));
		background: radial-gradient(ellipse at 50% 38%, var(--felt-hi), var(--felt) 72%);
		color: var(--felt-ink);
		overflow: hidden;
	}
	.felt::before {
		content: '';
		position: absolute;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		opacity: 0;
		transition: opacity 0.6s;
		pointer-events: none;
		z-index: 1;
	}
	.felt.dim::before {
		opacity: 1;
	}
	.felt::after {
		content: '';
		position: absolute;
		left: 50%;
		top: 40%;
		width: 200vmax;
		height: 200vmax;
		margin: -100vmax 0 0 -100vmax;
		background: repeating-conic-gradient(rgba(255, 196, 80, 0.22) 0 5deg, transparent 5deg 15deg);
		-webkit-mask: radial-gradient(circle, #000 0, transparent 30%);
		mask: radial-gradient(circle, #000 0, transparent 30%);
		opacity: 0;
		transition: opacity 0.5s;
		pointer-events: none;
		z-index: 1;
		animation: spin 18s linear infinite;
	}
	.felt.rays::after {
		opacity: 1;
	}
	.felt.rare-rays::after {
		background: repeating-conic-gradient(rgba(65, 228, 207, 0.18) 0 5deg, transparent 5deg 15deg);
	}
	@keyframes spin {
		to {
			rotate: 360deg;
		}
	}
	.open-head {
		width: 100%;
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 10px;
		position: relative;
		z-index: 2;
	}
	.open-head .eyebrow {
		color: rgba(238, 240, 255, 0.6);
	}
	.open-head h1 {
		font-size: 20px;
	}
	.open-ctrls {
		display: flex;
		gap: 8px;
		align-items: center;
		flex-wrap: wrap;
	}
	.felt .seg {
		background: rgba(255, 255, 255, 0.06);
		border-color: rgba(238, 240, 255, 0.2);
	}
	.felt .seg button {
		color: rgba(238, 240, 255, 0.7);
	}
	.felt .seg button[aria-pressed='true'] {
		background: var(--felt-ink);
		color: var(--felt);
	}
	.ghost-btn {
		border: 1px solid rgba(238, 240, 255, 0.25);
		background: rgba(255, 255, 255, 0.06);
		color: var(--felt-ink);
		border-radius: 10px;
		padding: 7px 12px;
		font-size: 12.5px;
		font-weight: 600;
	}
	.arena {
		position: relative;
		z-index: 2;
		display: flex;
		justify-content: center;
		align-items: center;
		outline: none;
		touch-action: none;
		max-width: 100%;
		margin-top: 6px;
	}
	.arena:focus-visible {
		outline: 2px dashed rgba(238, 240, 255, 0.5);
		outline-offset: 10px;
		border-radius: 12px;
	}
	.hint {
		position: relative;
		z-index: 2;
		font-size: 13.5px;
		color: rgba(238, 240, 255, 0.75);
		text-align: center;
		min-height: 20px;
	}

	/* tear strip */
	.tear-wrap {
		position: relative;
		width: var(--rw);
		height: var(--rh);
		margin-block: calc(var(--peek) * 0.7);
	}
	.tear-wrap.tilt {
		transform: perspective(900px) rotateX(calc(var(--rx, 0deg) * 0.6))
			rotateY(calc(var(--ry, 0deg) * 0.6));
	}
	.tear-pack {
		position: absolute;
		left: 50%;
		top: -6%;
		height: 112%;
		width: calc(var(--rw) * 0.978);
		margin-left: calc(var(--rw) * -0.489);
	}
	.tear-pack :global(.pack) {
		position: absolute;
		inset: 0;
		height: 100%;
	}
	.tp-body,
	.tp-flap {
		position: absolute;
		inset: 0;
	}
	.tp-body {
		clip-path: inset(12.5% -5% -5% -5%);
	}
	.tp-flap {
		clip-path: inset(-5% -5% 87.5% -5%);
		transform-origin: 100% 12.5%;
		transform: rotate(calc(var(--t) * -7deg)) translateY(calc(var(--t) * -4px));
	}
	.torn .tp-flap {
		transition:
			transform 0.7s cubic-bezier(0.3, 0.6, 0.4, 1),
			opacity 0.7s;
		transform: translate(60%, -160%) rotate(-38deg);
		opacity: 0;
	}
	.torn .tp-body {
		transition:
			transform 0.7s 0.25s cubic-bezier(0.5, 0, 0.7, 0.4),
			opacity 0.4s 0.55s;
		transform: translateY(80%);
		opacity: 0;
	}
	.tear-zone {
		position: absolute;
		left: -8%;
		right: -8%;
		top: 2%;
		height: 21%;
		cursor: grab;
		touch-action: none;
		z-index: 3;
	}
	.tear-guide {
		position: absolute;
		left: 8%;
		right: 8%;
		top: 50%;
		border-top: 2px dashed rgba(255, 255, 255, 0.55);
	}
	.tear-line {
		position: absolute;
		left: 8%;
		top: calc(50% - 2px);
		height: 5px;
		width: calc(var(--t) * 84%);
		background:
			linear-gradient(#fff, #fff) top/100% 2px no-repeat,
			repeating-linear-gradient(60deg, #fff 0 2px, transparent 2px 5px);
		filter: drop-shadow(0 0 4px rgba(255, 255, 255, 0.8));
	}
	.tear-hand {
		position: absolute;
		top: 50%;
		left: 8%;
		width: 26px;
		height: 26px;
		margin: -13px 0 0 -13px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.9);
		box-shadow: 0 0 0 6px rgba(255, 255, 255, 0.25);
		animation: swipehint 1.8s ease-in-out infinite;
		pointer-events: none;
	}
	.started .tear-hand {
		display: none;
	}
	@keyframes swipehint {
		0% {
			left: 8%;
			opacity: 0;
		}
		15% {
			opacity: 1;
		}
		75% {
			left: 88%;
			opacity: 1;
		}
		100% {
			left: 92%;
			opacity: 0;
		}
	}

	/* reels */
	.reels {
		display: flex;
		gap: 14px;
		justify-content: center;
	}
	.reel {
		position: relative;
		width: var(--rw);
		flex: none;
		border-radius: 16px;
	}
	.reel-win {
		position: relative;
		height: calc(var(--rh) + var(--peek) * 2);
		overflow: hidden;
		border-radius: 16px;
		background: rgba(0, 0, 0, 0.28);
		box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
		-webkit-mask: linear-gradient(
			transparent,
			#000 calc(var(--peek) * 0.9),
			#000 calc(100% - var(--peek) * 0.9),
			transparent
		);
		mask: linear-gradient(
			transparent,
			#000 calc(var(--peek) * 0.9),
			#000 calc(100% - var(--peek) * 0.9),
			transparent
		);
	}
	.rstrip {
		position: absolute;
		left: 0;
		right: 0;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: var(--gap);
		will-change: transform;
	}
	.cell {
		width: var(--rw);
		flex: none;
		transition: opacity 0.4s;
	}
	.reel.landed .cell:not(.win) {
		opacity: 0.3;
	}
	/* The landed card shows in the payline overlay instead */
	.reel.landed .cell.win {
		visibility: hidden;
	}
	.payline {
		position: absolute;
		left: 0;
		top: var(--peek);
		width: var(--rw);
		height: var(--rh);
		border-radius: calc(var(--rw) * 0.065) / calc(var(--rh) * 0.047);
		pointer-events: none;
		z-index: 2;
		transition: box-shadow 0.4s;
	}
	/* The card being tilted (and its badge) goes above the neighbouring reels */
	.reel:has(:global(.tilting)) .payline {
		z-index: 3;
	}
	.landed-card {
		pointer-events: auto;
		animation: landpop 0.45s cubic-bezier(0.2, 1.6, 0.4, 1);
	}
	@keyframes landpop {
		40% {
			scale: 1.06;
		}
	}
	.pay {
		position: absolute;
		top: 50%;
		margin-top: -8px;
		border-block: 8px solid transparent;
		z-index: 2;
	}
	.pay.l {
		left: -13px;
		border-left: 10px solid rgba(238, 240, 255, 0.75);
	}
	.pay.r {
		right: -13px;
		border-right: 10px solid rgba(238, 240, 255, 0.75);
	}
	.reels.all .pay {
		display: none;
	}
	/* The tell frames the card on the payline, not the whole slot */
	/* Glow only: the card's own rarity border is the one border */
	.reel.tell-rare .payline {
		box-shadow: 0 0 34px 8px rgba(65, 228, 207, 0.5);
	}
	.reel.tell-legendary .payline {
		box-shadow: 0 0 50px 14px rgba(255, 190, 70, 0.65);
	}
	.reel.tell-legendary {
		animation: tremble 1.2s ease-in-out infinite;
	}
	.reel.landed {
		animation: none;
	}
	@keyframes tremble {
		0%,
		70%,
		100% {
			translate: 0 0;
		}
		74% {
			translate: -3px 0;
		}
		78% {
			translate: 3px 0;
		}
		82% {
			translate: -2px 0;
		}
		86% {
			translate: 2px 0;
		}
	}
	.best {
		width: calc(var(--rw) * 0.8);
		margin-block: var(--peek);
	}

	/* tray + summary */
	.tray {
		position: relative;
		z-index: 2;
		display: flex;
		gap: 8px;
		justify-content: center;
		min-height: 96px;
		max-width: 100%;
		padding: 8px 4px;
		margin-top: auto;
	}
	.slotmini {
		width: 62px;
		flex: none;
		position: relative;
	}
	.slotmini.empty {
		border: 1.5px dashed rgba(238, 240, 255, 0.2);
		border-radius: 6px;
		aspect-ratio: 63/88;
	}
	.slotmini.pop {
		animation: pop 0.45s cubic-bezier(0.2, 1.6, 0.4, 1);
	}
	.slotmini :global(.c-badge) {
		font-size: 9px;
		padding: 3px 5px;
	}
	@media (max-width: 699px) {
		.felt {
			padding-bottom: 48px;
		}
		.tray {
			gap: 6px;
		}
		.slotmini {
			width: 50px;
		}
	}
	.after {
		position: relative;
		z-index: 2;
		width: min(460px, 100%);
		display: flex;
		flex-direction: column;
		gap: 12px;
		background: rgba(255, 255, 255, 0.07);
		border: 1px solid rgba(238, 240, 255, 0.14);
		border-radius: 18px;
		padding: 16px;
		backdrop-filter: blur(6px);
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 8px;
		text-align: center;
	}
	.stats b {
		display: block;
		font: 800 26px/1 var(--display);
	}
	.stats span {
		font-size: 11.5px;
		color: rgba(238, 240, 255, 0.65);
	}
	.row {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.row .btn {
		flex: 1;
	}
	.after .btn.primary {
		background: var(--felt-ink);
		color: var(--felt);
		border-color: transparent;
	}
	.after .btn:not(.primary) {
		background: transparent;
		color: var(--felt-ink);
		border-color: rgba(238, 240, 255, 0.3);
	}
	.empty {
		position: relative;
		z-index: 2;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		text-align: center;
		margin-top: 40px;
		max-width: 320px;
	}
	.empty h2 {
		font-size: 26px;
	}
	.empty p {
		color: rgba(238, 240, 255, 0.7);
	}
</style>
