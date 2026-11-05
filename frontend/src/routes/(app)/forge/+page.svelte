<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog, getToasts } from '#lib/session.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import { arp, burst, reducedMotion, sleep, tone } from '#lib/fx.ts';
	import {
		ABOVE,
		needleAngle,
		ROULETTE_CAP,
		ROULETTE_MAX_BURN,
		rouletteBase,
		rouletteBonus,
		rouletteChance,
		spare,
		TRADEUP_COST,
		tradeupOdds,
		wedge
	} from '#lib/forge/rules.ts';
	import type { CardDto, OwnedDto, Rarity } from '#lib/types.ts';
	import Card from '#lib/components/Card.svelte';
	import { foilOf } from '#lib/foil.ts';
	import HoldButton from '#lib/components/HoldButton.svelte';
	import CollectionPicker from '#lib/components/CollectionPicker.svelte';
	import RevealOverlay from '#lib/components/RevealOverlay.svelte';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const catalog = getCatalog()!;
	const toasts = getToasts();

	// Local copies: forging updates them without a reload
	let owned = $state<Record<string, OwnedDto>>({});
	let details = $state<Record<string, CardDto>>({});
	$effect(() => {
		owned = Object.fromEntries(data.binder.cards.map((c) => [c.card, { ...c }]));
		details = { ...(data.binder.details ?? {}) };
	});

	const tab = $derived(page.url.searchParams.get('tab') === 'roulette' ? 'roulette' : 'tradeup');
	const setTab = (t: string) => goto(t === 'roulette' ? '?tab=roulette' : '?', { replace: true });

	const RANK: Record<Rarity, number> = { common: 0, rare: 1, legendary: 2 };
	const rarityName = (r: Rarity) => i18n.t(`card.${r}`).toLowerCase();
	const pct = (bp: number) =>
		new Intl.NumberFormat(i18n.lang === 'fr' ? 'fr-FR' : 'en-US', {
			style: 'percent',
			maximumFractionDigits: 2
		}).format(bp / 10_000);
	const byRarity = (a: CardDto, b: CardDto) =>
		RANK[b.rarity] - RANK[a.rarity] || a.title.localeCompare(b.title);
	const cards = $derived(Object.values(details));

	function addOwned(slug: string, plain: number, foil = 0, pattern?: string) {
		const o = owned[slug] ?? { card: slug, count: 0, foils: 0, patterns: [] };
		o.count += plain + foil;
		o.foils += foil;
		if (foil < 0) o.patterns = o.patterns.slice(-foil);
		if (foil > 0 && pattern) o.patterns = [pattern, ...o.patterns];
		if (o.count <= 0) delete owned[slug];
		else owned[slug] = o;
	}
	async function ensureDetails(slug: string) {
		if (!details[slug]) details[slug] = await api<CardDto>(`/cards/${slug}`);
	}
	type Copy = { slug: string; foil: boolean };
	/** Cards sent to the API: one entry per card and finish, with a count */
	function grouped(copies: Copy[]) {
		const m = new Map<string, { card: string; foil: boolean; count: number }>();
		for (const c of copies) {
			const k = `${c.slug}|${c.foil}`;
			const e = m.get(k) ?? { card: c.slug, foil: c.foil, count: 0 };
			e.count++;
			m.set(k, e);
		}
		return [...m.values()];
	}

	// ---------------- set filter, shared by both trays ----------------
	let setFilter = $state<Record<'tradeup' | 'roulette', string>>({ tradeup: '', roulette: '' });
	let choosingSet = $state(false);
	const inSet = (c: CardDto, which: 'tradeup' | 'roulette') =>
		!setFilter[which] || c.collection === setFilter[which];
	/** Cards per collection in a tray, for the picker */
	const perSet = (list: CardDto[]) =>
		list.reduce<Record<string, number>>(
			(m, c) => ((m[c.collection] = (m[c.collection] ?? 0) + 1), m),
			{}
		);

	// ---------------- trade-up ----------------
	let tier = $state<'common' | 'rare'>('common');
	let slots = $state<Copy[]>([]);
	let result = $state<CardDto | null>(null);
	let pull = $state(0);
	let charging = $state(false);
	let core = $state<HTMLElement>();
	let lastTick = 0;
	const target = $derived(ABOVE[tier]!);
	const usedTU = (slug: string, foil = false) =>
		slots.filter((s) => s.slug === slug && s.foil === foil).length;
	/** Copies left to put in the ring: any copy, plain first */
	function freeTU(slug: string) {
		const o = owned[slug];
		if (!o) return { plain: 0, foil: 0 };
		return {
			plain: Math.max(0, o.count - o.foils - usedTU(slug)),
			foil: Math.max(0, o.foils - usedTU(slug, true))
		};
	}
	const leftTU = (slug: string) => freeTU(slug).plain + freeTU(slug).foil;
	// Every card of the tier: single copies too (AC-51), marked when it is the last one
	const tuAll = $derived(cards.filter((c) => c.rarity === tier && (owned[c.slug]?.count ?? 0) > 0));
	const tuTray = $derived(
		tuAll
			.filter((c) => inSet(c, 'tradeup'))
			.toSorted(
				(a, b) =>
					(owned[b.slug]?.count ?? 0) - (owned[a.slug]?.count ?? 0) ||
					a.title.localeCompare(b.title)
			)
	);
	const odds = $derived(
		tradeupOdds(
			slots.map((s) => ({ collection: details[s.slug].collection })),
			target,
			catalog.collections
		)
	);
	const pie = $derived.by(() => {
		let at = 0;
		const filled = (slots.length * 100) / TRADEUP_COST;
		const stops = odds.map((o) => {
			const from = at;
			at += o.share * filled;
			return `${catalog.get(o.collection)?.primary ?? '#888'} ${from}% ${at}%`;
		});
		if (at < 100) stops.push(`var(--surface-2) ${at}% 100%`);
		return `conic-gradient(${stops.join(',')})`;
	});

	function addSlot(slug: string) {
		const f = freeTU(slug);
		if (slots.length >= TRADEUP_COST || f.plain + f.foil < 1) return;
		result = null;
		slots.push({ slug, foil: f.plain < 1 });
		tone(400 + slots.length * 50, 0.05, 'triangle', 0.04);
	}
	function autofill() {
		result = null;
		while (slots.length < TRADEUP_COST) {
			const best = cards
				.filter(
					(c) =>
						c.rarity === tier && inSet(c, 'tradeup') && spare(owned[c.slug], usedTU(c.slug)) > 0
				)
				.toSorted(
					(a, b) => spare(owned[b.slug], usedTU(b.slug)) - spare(owned[a.slug], usedTU(a.slug))
				)[0];
			if (!best) break;
			slots.push({ slug: best.slug, foil: false });
		}
		if (slots.length < TRADEUP_COST)
			toasts.show(i18n.t('forge.autofill_short', { n: slots.length }));
	}
	async function fuse() {
		charging = false;
		lastTick = 0;
		const spent = [...slots];
		try {
			const res = await post<{ card: string; is_new: boolean }>('/forge/tradeup', {
				cards: grouped(spent)
			});
			for (const c of spent) addOwned(c.slug, c.foil ? 0 : -1, c.foil ? -1 : 0);
			await ensureDetails(res.card);
			addOwned(res.card, 1);
			slots = [];
			pull = 0;
			result = details[res.card];
			arp([392, 523, 659, 784, 1046], 0.07, 'triangle', 0.06);
			setTimeout(() => core && burst(core, target === 'legendary' ? '#ffcf5c' : '#41e4cf', 34));
			reveal = {
				card: details[res.card],
				foil: null,
				eyebrow: i18n.t('forge.reveal_forged'),
				detail: i18n.t('forge.reveal_forged_detail', { n: TRADEUP_COST }),
				isNew: res.is_new
			};
		} catch (e) {
			pull = 0;
			toasts.show(i18n.error(e), 'warn');
		}
	}

	// ---------------- roulette ----------------
	let rTarget = $state<string | null>(null);
	let burn = $state<Copy[]>([]);
	let choosing = $state(false);
	let spinning = $state(false);
	let rot = $state(0);
	let spinMs = $state(0);
	let shown = $state(0);
	let outcome = $state<{ won: boolean; slug: string; burned: number } | null>(null);
	let dial = $state<HTMLElement>();

	const plainOf = (slug: string) => (owned[slug] ? owned[slug].count - owned[slug].foils : 0);
	const burnedOf = (slug: string, foil: boolean) =>
		burn.filter((b) => b.slug === slug && b.foil === foil).length;
	/** Plain copies left to burn, except the one being rolled. Foils never burn. */
	const leftRO = (slug: string) =>
		Math.max(0, plainOf(slug) - burnedOf(slug, false) - (slug === rTarget ? 1 : 0));
	/** Burning one more copy would leave none of the card, foils included */
	const lastCopy = (slug: string) =>
		slug !== rTarget && leftRO(slug) === 1 && (owned[slug]?.foils ?? 0) === 0;
	// Starts on the rarest card the player can roll
	$effect(() => {
		if (rTarget && plainOf(rTarget) < 1 && !spinning) rTarget = null;
		if (!rTarget && !choosing && !outcome)
			rTarget = cards.filter((c) => plainOf(c.slug) > 0).toSorted(byRarity)[0]?.slug ?? null;
	});
	const tCard = $derived(rTarget ? details[rTarget] : null);
	const picking = $derived(choosing || !tCard);
	const burnedRarities = $derived(burn.map((b) => details[b.slug].rarity));
	const chance = $derived(tCard ? rouletteChance(tCard.rarity, burnedRarities) : 0);
	const roAll = $derived(
		picking
			? cards.filter((c) => plainOf(c.slug) > 0).toSorted(byRarity)
			: cards.filter((c) => leftRO(c.slug) > 0).toSorted(byRarity)
	);
	const roTray = $derived(roAll.filter((c) => inSet(c, 'roulette')));
	const bonusLines = $derived(
		(['common', 'rare', 'legendary'] as const)
			.map((r) => ({ r, n: burnedRarities.filter((x) => x === r).length }))
			.filter((l) => l.n > 0)
	);
	// The slice grows smoothly as cards are burned
	$effect(() => {
		const to = chance;
		const from = shown;
		const t0 = performance.now();
		let raf = 0;
		const step = (t: number) => {
			const p = Math.min(1, (t - t0) / 350);
			shown = from + (to - from) * (1 - Math.pow(1 - p, 3));
			if (p < 1) raf = requestAnimationFrame(step);
		};
		raf = requestAnimationFrame(step);
		return () => cancelAnimationFrame(raf);
	});
	const ticks = Array.from({ length: 20 }, (_, i) => {
		const a = (i / 20) * Math.PI * 2;
		const [r1, r2] = i % 5 ? [100, 104] : [98, 108];
		return {
			x1: Math.sin(a) * r1,
			y1: -Math.cos(a) * r1,
			x2: Math.sin(a) * r2,
			y2: -Math.cos(a) * r2
		};
	});
	const labels = [0, 25, 50, 75].map((v) => {
		const a = (v / 100) * Math.PI * 2;
		return { v, x: Math.sin(a) * 118, y: -Math.cos(a) * 118 + 2.5 };
	});

	function pickTarget(slug: string) {
		rTarget = slug;
		burn = burn.filter((b) => b.slug !== slug);
		choosing = false;
		outcome = null;
	}
	function addBurn(slug: string) {
		if (spinning || leftRO(slug) < 1) return;
		if (chance >= ROULETTE_CAP) return toasts.show(i18n.t('forge.at_cap'));
		if (burn.length >= ROULETTE_MAX_BURN) return;
		outcome = null;
		burn.push({ slug, foil: false });
		tone(250, 0.08, 'sawtooth', 0.03);
	}
	/** Whether the whole dial is on screen, below the header */
	function dialInView() {
		if (!dial) return true;
		const r = dial.getBoundingClientRect();
		const top = document.querySelector('header.top')?.getBoundingClientRect().bottom ?? 0;
		// The bottom tab bar on phones covers the end of the screen
		const bar = document.querySelector('nav.nav')?.getBoundingClientRect();
		const bottom = bar && bar.top > window.innerHeight / 2 ? bar.top : window.innerHeight;
		return r.top >= top && r.bottom <= bottom;
	}
	async function spin() {
		if (!rTarget || spinning) return;
		spinning = true;
		outcome = null;
		// Phones: the button sits below the dial; bring the dial back in view to watch the spin
		const shown = dialInView();
		if (!shown)
			dial?.scrollIntoView({ block: 'center', behavior: reducedMotion() ? 'auto' : 'smooth' });
		const slug = rTarget;
		const spent = [...burn];
		let res: { won: boolean; chance: number; foil: string | null };
		try {
			res = await post('/forge/roulette', { target: slug, burn: grouped(spent) });
		} catch (e) {
			spinning = false;
			toasts.show(i18n.error(e), 'warn');
			return;
		}
		// Let the scroll land before the needle moves
		if (!shown && !reducedMotion()) await sleep(450);
		// The server decided: the needle only shows it
		spinMs = reducedMotion() ? 10 : 4200;
		rot = Math.ceil(rot / 360) * 360 + 360 * 5 + needleAngle(res.won, res.chance);
		if (!reducedMotion()) {
			for (let t = 0, gap = 35; t < 3900; t += gap, gap *= 1.075)
				setTimeout(() => tone(1100, 0.012, 'square', 0.02), t);
		}
		await sleep(spinMs + 100);
		for (const b of spent) addOwned(b.slug, b.foil ? 0 : -1, b.foil ? -1 : 0);
		if (res.won) addOwned(slug, -1, 1, res.foil ?? undefined);
		else addOwned(slug, -1);
		burn = [];
		outcome = { won: res.won, slug, burned: spent.length };
		if (res.won) {
			arp([784, 988, 1175, 1568, 1976], 0.08, 'triangle', 0.06);
			if (dial) burst(dial, '#ffcf5c', 40);
			wonFoil = res.foil ?? 'foil3';
			reveal = {
				card: details[slug],
				foil: wonFoil,
				eyebrow: i18n.t('forge.reveal_foil'),
				detail: i18n.t('forge.reveal_foil_detail', { p: pct(res.chance) })
			};
		} else {
			tone(160, 0.4, 'sawtooth', 0.04);
			rTarget = null;
			choosing = true;
		}
		spinning = false;
	}
	const shownCard = $derived(outcome ? details[outcome.slug] : tCard);
	/** The won foil pattern, for the card shown on the dial */
	let wonFoil = $state<string | null>(null);

	// ---------------- full-screen reveal ----------------
	let reveal = $state<{
		card: CardDto;
		foil: string | null;
		eyebrow: string;
		detail: string;
		isNew?: boolean;
	} | null>(null);
</script>

<svelte:head><title>{i18n.t('nav.forge')} · AnyCardz</title></svelte:head>

{#snippet setButton()}
	{@const set = setFilter[tab]}
	<button
		type="button"
		class="btn sm set-filter"
		aria-haspopup="dialog"
		onclick={() => (choosingSet = true)}
		aria-label="{i18n.t('forge.set_filter')}: {set
			? catalog.get(set)?.title
			: i18n.t('forge.all_sets')}"
	>
		{#if set}<i style:background={catalog.get(set)?.primary}></i>{/if}
		{set ? catalog.get(set)?.title : i18n.t('forge.all_sets')}<span aria-hidden="true">▾</span>
	</button>
{/snippet}

{#snippet showAll()}
	<p class="emptynote">
		{i18n.t('forge.none_in_set')}
		<button type="button" class="linkish" onclick={() => (setFilter[tab] = '')}
			>{i18n.t('forge.show_all')}</button
		>
	</p>
{/snippet}

{#if reveal}
	<RevealOverlay {...reveal} onClose={() => (reveal = null)} />
{/if}

{#if choosingSet}
	{@const counts = perSet(tab === 'tradeup' ? tuAll : roAll)}
	<CollectionPicker
		collections={catalog.collections.filter((c) => counts[c.slug])}
		completion={data.binder.collections}
		{counts}
		current={setFilter[tab]}
		allLabel={i18n.t('forge.all_sets')}
		allCount={(tab === 'tradeup' ? tuAll : roAll).length}
		onPick={(slug) => {
			setFilter[tab] = slug;
			choosingSet = false;
		}}
		onClose={() => (choosingSet = false)}
	/>
{/if}

<div class="screen">
	<div class="seg tabs" role="group" aria-label={i18n.t('forge.mode')}>
		<button type="button" aria-pressed={tab === 'tradeup'} onclick={() => setTab('tradeup')}
			>{i18n.t('forge.tradeup_tab')}</button
		>
		<button type="button" aria-pressed={tab === 'roulette'} onclick={() => setTab('roulette')}
			>{i18n.t('forge.roulette_tab')}</button
		>
	</div>

	{#if tab === 'tradeup'}
		<div class="grid">
			<div class="ring-wrap" class:charging style:--pull={pull}>
				<div class="ring">
					{#each Array(TRADEUP_COST) as _, i (i)}
						{@const s = slots[i]}
						{#if s}
							<button
								type="button"
								class="slot"
								style:--a="{i * 36}deg"
								onclick={() => slots.splice(i, 1)}
								aria-label={i18n.t('forge.slot_remove', { name: details[s.slug].title })}
							>
								<Card card={details[s.slug]} foil={s.foil ? foilOf(owned[s.slug]) : null} />
							</button>
						{:else}
							<span class="slot" style:--a="{i * 36}deg"
								><span class="empty-slot" aria-hidden="true">{i + 1}</span></span
							>
						{/if}
					{/each}
				</div>
				<div class="core" bind:this={core} style:background={pie}>
					{#if result}
						<div class="core-card"><Card card={result} tilt /></div>
					{:else}
						<div class="core-in">
							<b>{slots.length}/{TRADEUP_COST}</b><span
								>{i18n.t(`card.${tier}`)} → {i18n.t(`card.${target}`)}</span
							>
						</div>
					{/if}
				</div>
			</div>

			<div class="side">
				<div>
					<p class="eyebrow">{i18n.t('forge.tradeup_eyebrow')}</p>
					<h1>
						{i18n.t('forge.tradeup_title', { from: rarityName(tier), to: rarityName(target) })}
					</h1>
				</div>
				<p class="lead">{i18n.t('forge.tradeup_lead')}</p>
				<div class="seg" role="group" aria-label={i18n.t('forge.tier')}>
					{#each ['common', 'rare'] as const as t (t)}
						<button
							type="button"
							aria-pressed={tier === t}
							onclick={() => {
								tier = t;
								slots = [];
								result = null;
							}}>{i18n.t(`forge.tier_${t}`)}</button
						>
					{/each}
				</div>
				<ul class="odds" aria-live="polite">
					{#each odds as o (o.collection)}
						<li>
							<i style:background={catalog.get(o.collection)?.primary}></i>{catalog.get(
								o.collection
							)?.title ?? o.collection}<b class="mono">{Math.round(o.share * 100)}%</b>
						</li>
					{:else}
						<li class="none">
							{slots.length
								? i18n.t('forge.no_payout', { to: rarityName(target) })
								: i18n.t('forge.odds_empty')}
						</li>
					{/each}
				</ul>
				<HoldButton
					class="btn fuse"
					ms={1500}
					disabled={slots.length < TRADEUP_COST || !odds.length}
					onProgress={(p) => {
						charging = true;
						pull = p * p;
						const s = Math.floor(p * 14);
						if (s > lastTick) {
							lastTick = s;
							tone(200 + s * 60, 0.05, 'sawtooth', 0.025);
						}
					}}
					onCancel={() => {
						charging = false;
						pull = 0;
						lastTick = 0;
					}}
					onDone={fuse}
				>
					<span
						>{slots.length < TRADEUP_COST
							? i18n.t('forge.add_more', { n: TRADEUP_COST - slots.length })
							: i18n.t('forge.hold_fuse')}</span
					>
				</HoldButton>
				<div class="row">
					<button type="button" class="btn sm" onclick={autofill}>{i18n.t('forge.autofill')}</button
					>
					<button type="button" class="btn sm" onclick={() => (slots = [])} disabled={!slots.length}
						>{i18n.t('forge.clear')}</button
					>
				</div>
			</div>
		</div>

		<div class="tray-head">
			<div>
				<h2>{i18n.t('forge.spare_title', { rarity: rarityName(tier) })}</h2>
				<p>{i18n.t('forge.spare_hint')}</p>
			</div>
			{@render setButton()}
		</div>
		<div class="tray">
			{#each tuTray as c (c.slug)}
				{@const f = freeTU(c.slug)}
				{@const left = f.plain + f.foil}
				<button
					type="button"
					class="tcard"
					onclick={() => addSlot(c.slug)}
					disabled={slots.length >= TRADEUP_COST || left < 1}
					aria-label={i18n.t('forge.add', { name: c.title })}
				>
					<Card
						card={c}
						foil={f.plain < 1 && f.foil > 0 ? foilOf(owned[c.slug], usedTU(c.slug, true)) : null}
					>
						{#snippet badge()}
							<span class="c-badge">×{left}</span>
							{#if left === 1}<span class="c-badge last">{i18n.t('forge.last_copy')}</span>
							{:else if f.plain < 1 && f.foil > 0}<span class="c-badge foilb"
									>{i18n.t('card.foil')}</span
								>{/if}
						{/snippet}
					</Card>
				</button>
			{:else}
				{#if setFilter.tradeup && tuAll.length}
					{@render showAll()}
				{:else}
					<p class="emptynote">{i18n.t('forge.spare_none', { rarity: rarityName(tier) })}</p>
				{/if}
			{/each}
		</div>
	{:else}
		<div class="grid">
			<div
				class="dial-wrap"
				bind:this={dial}
				class:win={outcome?.won}
				class:lose={outcome && !outcome.won}
			>
				<svg class="dial" viewBox="-125 -125 250 250" aria-hidden="true">
					<circle class="d-bg" r="96" />
					<path class="d-win" d={wedge(shown / 100)} />
					{#each ticks as t, i (i)}<line
							class="d-tick"
							x1={t.x1}
							y1={t.y1}
							x2={t.x2}
							y2={t.y2}
						/>{/each}
					{#each labels as l (l.v)}<text class="d-lbl" x={l.x} y={l.y} text-anchor="middle"
							>{l.v}%</text
						>{/each}
					<g
						class="needle"
						style:transform="rotate({rot}deg)"
						style:transition="transform {spinMs}ms cubic-bezier(.12,.72,.1,1)"
					>
						<polygon points="0,-100 7,-6 -7,-6" /><circle r="9" />
					</g>
				</svg>
				{#if shownCard}
					<div class="dial-target" class:burnt={outcome && !outcome.won}>
						<Card
							card={shownCard}
							foil={outcome?.won ? (wonFoil ?? 'foil3') : null}
							tilt={!outcome || outcome.won}
						/>
					</div>
				{/if}
			</div>

			<div class="side">
				<div>
					<p class="eyebrow">{i18n.t('forge.roulette_eyebrow')}</p>
					<h1>
						{#if outcome && shownCard}
							{i18n.t(outcome.won ? 'forge.roulette_won' : 'forge.roulette_lost', {
								name: shownCard.title
							})}
						{:else if tCard}
							{i18n.t('forge.roulette_title', { name: tCard.title })}
						{:else}
							{i18n.t('forge.roulette_pick')}
						{/if}
					</h1>
				</div>
				<p class="lead">{i18n.t('forge.roulette_lead')}</p>
				{#if tCard && !outcome}
					<p class="math" aria-live="polite">
						{i18n.t('forge.math_base', {
							p: pct(rouletteBase(tCard.rarity)),
							rarity: rarityName(tCard.rarity)
						})}
						{#each bonusLines as l (l.r)}
							+ {i18n.t('forge.math_bonus', {
								n: l.n,
								rarity: rarityName(l.r),
								p: pct(rouletteBonus(l.r))
							})}
						{/each}
						<br />{i18n.t('forge.math_total', { p: pct(chance) })}
						{#if chance >= ROULETTE_CAP}{i18n.t('forge.math_capped')}{/if}
						<br /><span class="stake"
							>{burn.length
								? i18n.t('forge.math_stake_burn', { name: tCard.title, n: burn.length })
								: i18n.t('forge.math_stake', { name: tCard.title })}</span
						>
					</p>
				{/if}
				{#if outcome && shownCard}
					<p class="banner" class:won={outcome.won} role="status">
						{outcome.won
							? i18n.t('forge.won', { name: shownCard.title })
							: outcome.burned
								? i18n.t('forge.lost', { name: shownCard.title, n: outcome.burned })
								: i18n.t('forge.lost_alone', { name: shownCard.title })}
					</p>
				{/if}
				{#if !picking}
					<div>
						<p class="eyebrow">{i18n.t('forge.burning', { n: burn.length })}</p>
						<div class="burns">
							{#each burn as b, i (`${i}-${b.slug}-${b.foil}`)}
								<button
									type="button"
									class="tcard mini"
									onclick={() => burn.splice(i, 1)}
									disabled={spinning}
									aria-label={i18n.t('forge.unburn', { name: details[b.slug].title })}
									><Card
										card={details[b.slug]}
										foil={b.foil ? foilOf(owned[b.slug]) : null}
									/></button
								>
							{:else}
								<span class="muted">{i18n.t('forge.burn_hint')}</span>
							{/each}
						</div>
					</div>
				{/if}
				<div class="row">
					<HoldButton
						class="btn accent spin"
						fill="var(--legend)"
						ms={900}
						disabled={!tCard || spinning || choosing}
						onDone={spin}
					>
						<span>{i18n.t('forge.hold_spin', { p: pct(chance) })}</span>
					</HoldButton>
					<button
						type="button"
						class="btn sm"
						onclick={() => {
							choosing = !choosing;
							outcome = null;
						}}
						disabled={spinning}
					>
						{choosing ? i18n.t('forge.back_to_burning') : i18n.t('forge.change_card')}
					</button>
				</div>
			</div>
		</div>

		<div class="tray-head">
			<div>
				<h2>{picking ? i18n.t('forge.target_title') : i18n.t('forge.burn_title')}</h2>
				<p>{picking ? i18n.t('forge.target_hint') : i18n.t('forge.burn_odds')}</p>
			</div>
			{@render setButton()}
		</div>
		<div class="tray">
			{#each roTray as c (c.slug)}
				<button
					type="button"
					class="tcard"
					onclick={() => (picking ? pickTarget(c.slug) : addBurn(c.slug))}
					disabled={spinning}
					aria-label={c.title}
				>
					<Card card={c}>
						{#snippet badge()}
							{#if !picking}
								<span class="c-badge">×{leftRO(c.slug)}</span>
								{#if lastCopy(c.slug)}<span class="c-badge last">{i18n.t('forge.last_copy')}</span
									>{/if}
							{/if}
						{/snippet}
					</Card>
				</button>
			{:else}
				{#if setFilter.roulette && roAll.length}
					{@render showAll()}
				{:else}
					<p class="emptynote">{i18n.t('forge.nothing')}</p>
				{/if}
			{/each}
		</div>
	{/if}
</div>

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
	.tabs {
		align-self: flex-start;
	}
	.grid {
		display: grid;
		gap: 20px;
		grid-template-columns: minmax(0, 1fr);
		align-items: center;
	}
	@media (min-width: 900px) {
		.grid {
			grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		}
	}
	.side {
		display: flex;
		flex-direction: column;
		gap: 14px;
		min-width: 0;
	}
	.side > .seg {
		align-self: flex-start;
	}
	h1 {
		font-size: 24px;
	}
	.lead {
		color: var(--muted);
		max-width: 46ch;
	}
	.row {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
		align-items: center;
	}

	/* trade-up ring */
	.ring-wrap {
		position: relative;
		width: min(100%, 390px);
		aspect-ratio: 1;
		margin: 0 auto;
		container-type: inline-size;
		--pull: 0;
	}
	.ring-wrap::before {
		content: '';
		position: absolute;
		inset: 6%;
		border-radius: 50%;
		border: 1.5px dashed var(--line);
	}
	.ring {
		position: absolute;
		inset: 0;
		transform: rotate(calc(var(--pull) * 600deg));
		transition: transform 0.6s cubic-bezier(0.2, 1.3, 0.4, 1);
	}
	.charging .ring,
	.charging .slot {
		transition: none;
	}
	.slot {
		position: absolute;
		left: 50%;
		top: 50%;
		width: 16cqi;
		aspect-ratio: 63/88;
		border: 0;
		padding: 0;
		background: none;
		transform: translate(-50%, -50%) rotate(var(--a))
			translateY(calc(-37cqi * (1 - var(--pull) * 0.82))) rotate(calc(var(--a) * -1));
		transition: transform 0.6s cubic-bezier(0.2, 1.3, 0.4, 1);
	}
	.slot :global(.card) {
		animation: pop 0.35s cubic-bezier(0.2, 1.6, 0.4, 1);
	}
	.empty-slot {
		position: absolute;
		inset: 0;
		border-radius: 6.5%/4.7%;
		border: 1.5px dashed var(--muted);
		opacity: 0.5;
		display: grid;
		place-items: center;
		font: 700 3.4cqi var(--mono);
		color: var(--muted);
	}
	.core {
		position: absolute;
		left: 50%;
		top: 50%;
		width: 42cqi;
		aspect-ratio: 1;
		translate: -50% -50%;
		border-radius: 50%;
		display: grid;
		place-items: center;
	}
	.charging .core {
		scale: calc(1 + var(--pull) * 0.15);
		box-shadow: 0 0 calc(var(--pull) * 60px) calc(var(--pull) * 20px)
			color-mix(in srgb, var(--accent) 60%, transparent);
	}
	.core-in {
		width: 80%;
		aspect-ratio: 1;
		border-radius: 50%;
		background: var(--surface);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		box-shadow: var(--shadow);
		text-align: center;
	}
	.core-in b {
		font: 800 9cqi/1 var(--display);
	}
	.core-in span {
		font-size: 3.4cqi;
		color: var(--muted);
		font-weight: 600;
	}
	.core-card {
		width: 22cqi;
		animation: reveal 0.7s cubic-bezier(0.2, 1.4, 0.4, 1);
	}
	@keyframes reveal {
		from {
			transform: scale(0.2) rotateY(180deg);
			opacity: 0;
		}
	}
	.odds {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.odds li {
		display: grid;
		grid-template-columns: 12px minmax(0, 1fr) auto;
		gap: 8px;
		align-items: center;
		font-size: 13.5px;
	}
	.odds li i {
		width: 12px;
		height: 12px;
		border-radius: 3px;
	}
	.odds .none {
		display: block;
		color: var(--muted);
		font-size: 13px;
	}
	.side :global(.fuse) {
		padding: 14px 18px;
		font-size: 15px;
		background: var(--ink);
		color: var(--bg);
		border-color: var(--ink);
	}

	/* roulette dial */
	.dial-wrap {
		position: relative;
		width: min(100%, 340px);
		aspect-ratio: 1;
		margin: 0 auto;
	}
	.dial {
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.d-bg {
		fill: var(--surface);
		stroke: var(--line);
		stroke-width: 1.5;
	}
	.d-win {
		fill: color-mix(in srgb, var(--legend) 85%, transparent);
	}
	.d-tick {
		stroke: var(--muted);
		stroke-width: 1;
	}
	.d-lbl {
		fill: var(--muted);
		font: 600 7px var(--mono);
	}
	.needle polygon {
		fill: var(--ink);
	}
	.needle circle {
		fill: var(--ink);
		stroke: var(--surface);
		stroke-width: 3;
	}
	.dial-target {
		position: absolute;
		left: 50%;
		top: 50%;
		width: 26%;
		translate: -50% -50%;
	}
	.dial-target.burnt :global(.cf) {
		filter: grayscale(1) brightness(0.4) contrast(1.3);
	}
	.dial-target.burnt::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(135deg, transparent 46%, var(--danger) 47% 53%, transparent 54%);
	}
	.dial-wrap.win .d-win {
		animation: winflash 0.25s 4 alternate;
	}
	@keyframes winflash {
		to {
			fill: #fff;
		}
	}
	.dial-wrap.lose {
		animation: shake 0.1s 4;
	}
	@keyframes shake {
		50% {
			translate: 3px 0;
		}
	}
	.math {
		font: 500 13px/1.6 var(--mono);
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 10px 12px;
		overflow-wrap: anywhere;
	}
	.stake {
		color: var(--danger);
	}
	.banner {
		padding: 12px 14px;
		border-radius: 12px;
		font-weight: 600;
		background: var(--surface-2);
		color: var(--muted);
		box-shadow: inset 0 0 0 1px var(--line);
	}
	.banner.won {
		color: var(--ink);
		background: color-mix(in srgb, var(--legend) 18%, var(--surface));
		box-shadow: inset 0 0 0 1.5px var(--legend);
	}
	.burns {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		min-height: 48px;
		margin-top: 6px;
		align-items: center;
	}
	.muted {
		color: var(--muted);
		font-size: 13px;
	}

	/* trays */
	.tray-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: 10px;
		flex-wrap: wrap;
	}
	.set-filter {
		gap: 6px;
		max-width: 100%;
	}
	.set-filter i {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		flex: none;
	}
	.tray :global(.c-badge.last) {
		top: auto;
		bottom: -6px;
		right: 50%;
		translate: 50% 0;
		white-space: nowrap;
		background: var(--warn);
		color: #1b1200;
		font-size: 9.5px;
		text-transform: uppercase;
	}
	.tray-head h2 {
		font-size: 17px;
	}
	.tray-head p {
		font-size: 12.5px;
		color: var(--muted);
	}
	.tray {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
		gap: 12px;
		padding: 4px 2px;
	}
	.tcard {
		position: relative;
		border: 0;
		background: none;
		padding: 0;
		display: block;
		transition: transform 0.2s;
	}
	/* The rarity as a frame: the last copy label covers the line that names it */
	.tray :global(.card) {
		border-radius: 6.5%/4.7%;
		box-shadow: 0 0 0 2px var(--rc, var(--common));
	}
	.tray :global(.card.r-common) {
		--rc: var(--common);
	}
	.tray :global(.card.r-rare) {
		--rc: var(--rare);
	}
	.tray :global(.card.r-legendary) {
		--rc: var(--legend);
	}
	.tcard:hover {
		transform: translateY(-4px);
	}
	.tcard:disabled {
		opacity: 0.35;
		cursor: not-allowed;
		transform: none;
	}
	.tcard.mini {
		width: 36px;
	}
	.emptynote {
		grid-column: 1 / -1;
		color: var(--muted);
		font-size: 13px;
		padding: 16px;
		border: 1.5px dashed var(--line);
		border-radius: 12px;
		text-align: center;
	}
</style>
