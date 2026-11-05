<script lang="ts">
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { api, ApiError, post } from '#lib/api.ts';
	import { arp, sleep, tone } from '#lib/fx.ts';
	import { change, diff, onSide, type Diff, type Line } from '#lib/trade/offer.ts';
	import type { CardDto, MeDto, OwnedDto, TradeDto } from '#lib/types.ts';
	import Card from '#lib/components/Card.svelte';
	import { foilOf } from '#lib/foil.ts';
	import SlideConfirm from '#lib/components/SlideConfirm.svelte';
	import { getLive } from '#lib/live.svelte.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	const live = getLive();

	// svelte-ignore state_referenced_locally
	let trade = $state<TradeDto>(data.trade);
	let busy = $state(false);
	let drawer = $state<'theirs' | 'mine'>('theirs');
	/** What the partner changed last: highlighted until our next edit */
	let changes = $state<{ mine: Diff; theirs: Diff } | null>(null);
	/** Seals cracking right now, per side */
	let broken = $state({ mine: false, theirs: false });
	let swapping = $state(false);

	const mineOwned = $derived(
		Object.fromEntries(data.mine.cards.map((c) => [c.card, c])) as Record<string, OwnedDto>
	);
	const theirsOwned = $derived(
		Object.fromEntries(data.theirs.cards.map((c) => [c.card, c])) as Record<string, OwnedDto>
	);
	const details = $derived<Record<string, CardDto>>({
		...(data.mine.details ?? {}),
		...data.theirs.details,
		...trade.details
	});
	const open = $derived(trade.state === 'open');
	const partner = $derived(trade.partner);

	/** Copies still free to put on a side: plain first, then foil */
	function free(owned: Record<string, OwnedDto>, lines: Line[], card: string) {
		const o = owned[card];
		if (!o) return { plain: 0, foil: 0 };
		return {
			plain: o.count - o.foils - onSide(lines, card, false),
			foil: o.foils - onSide(lines, card, true)
		};
	}

	/** A new offer state from the server: spot what the partner changed, crack broken seals */
	function apply(next: TradeDto) {
		const prev = trade;
		if (next.version !== prev.version && next.edited_by === 'partner') {
			changes = {
				mine: diff(prev.mine.cards, next.mine.cards),
				theirs: diff(prev.theirs.cards, next.theirs.cards)
			};
			if (prev.mine.sealed || prev.theirs.sealed) {
				crack(prev);
				tone(180, 0.2, 'sawtooth', 0.05);
				toasts.show(i18n.t('trade.partner_changed', { name: next.partner }), 'warn');
			}
		}
		if (!prev.theirs.sealed && next.theirs.sealed && next.state === 'open') {
			tone(140, 0.15, 'square', 0.05);
			toasts.show(i18n.t('trade.they_sealed', { name: next.partner }));
		}
		if (prev.state === 'open' && next.state === 'done') celebrate(next);
		trade = next;
	}

	function crack(t: TradeDto) {
		broken = { mine: t.mine.sealed, theirs: t.theirs.sealed };
		setTimeout(() => (broken = { mine: false, theirs: false }), 600);
	}

	async function celebrate(t: TradeDto) {
		swapping = true;
		arp([523, 659, 784], 0.1);
		await sleep(800);
		swapping = false;
		const me = await api<MeDto>('/me').catch(() => null);
		if (me) session.update(me);
		const n = (lines: Line[]) => lines.reduce((s, l) => s + l.count, 0);
		toasts.show(
			i18n.t('trade.swapped', {
				got: i18n.t('trade.swapped_cards', { n: n(t.theirs.cards) }),
				gave: i18n.t('trade.swapped_cards', { n: n(t.mine.cards) })
			})
		);
	}

	/** Saves the offer, based on the version we have */
	async function save(next: { give: Line[]; get: Line[]; give_coins: number; get_coins: number }) {
		if (busy || !open) return;
		busy = true;
		changes = null;
		try {
			const res = await api<TradeDto>(`/trades/${trade.id}`, {
				method: 'PUT',
				json: { version: trade.version, ...next }
			});
			if (trade.mine.sealed || trade.theirs.sealed) crack(trade);
			trade = res;
			tone(500, 0.04, 'triangle', 0.03);
		} catch (e) {
			if (e instanceof ApiError && e.code === 'trade_changed') {
				await refresh();
				toasts.show(i18n.t('trade.raced', { name: partner }), 'warn');
			} else toasts.show(i18n.error(e), 'warn');
		} finally {
			busy = false;
		}
	}
	const current = () => ({
		give: trade.mine.cards,
		get: trade.theirs.cards,
		give_coins: trade.mine.coins,
		get_coins: trade.theirs.coins
	});
	function add(side: 'mine' | 'theirs', card: string) {
		const f = free(side === 'mine' ? mineOwned : theirsOwned, trade[side].cards, card);
		const foil = f.plain > 0 ? false : f.foil > 0 ? true : null;
		if (foil === null) return;
		const next = current();
		if (side === 'mine') next.give = change(next.give, card, foil, 1);
		else next.get = change(next.get, card, foil, 1);
		save(next);
	}
	function remove(side: 'mine' | 'theirs', l: Line) {
		const next = current();
		if (side === 'mine') next.give = change(next.give, l.card, l.foil, -1);
		else next.get = change(next.get, l.card, l.foil, -1);
		save(next);
	}
	function setCoins(side: 'mine' | 'theirs', value: string) {
		const n = Math.max(0, Math.floor(Number(value) || 0));
		if (n === trade[side].coins) return;
		const next = current();
		if (side === 'mine') next.give_coins = n;
		else next.get_coins = n;
		save(next);
	}
	async function seal() {
		try {
			const res = await post<TradeDto>(`/trades/${trade.id}/seal`, { version: trade.version });
			tone(140, 0.15, 'square', 0.05);
			apply(res);
		} catch (e) {
			if (e instanceof ApiError && e.code === 'trade_changed') await refresh();
			toasts.show(i18n.error(e), 'warn');
		}
	}
	async function cancel() {
		try {
			trade = await post<TradeDto>(`/trades/${trade.id}/cancel`);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
	async function refresh() {
		const next = await api<TradeDto>(`/trades/${trade.id}`).catch(() => null);
		if (next) apply(next);
	}

	// The live socket says when the trade changes; a slow poll covers a dropped socket
	onMount(() => {
		const off = live.onTrade((id) => id === trade.id && !busy && refresh());
		const t = setInterval(() => {
			if (open && !busy && document.visibilityState === 'visible' && !live.connected) refresh();
		}, 2000);
		const slow = setInterval(() => open && !busy && refresh(), 15_000);
		return () => {
			off();
			clearInterval(t);
			clearInterval(slow);
		};
	});

	const count = (lines: Line[]) => lines.reduce((s, l) => s + l.count, 0);
	const isAdded = (side: 'mine' | 'theirs', l: Line) =>
		!!changes?.[side].added.some((a) => a.card === l.card && a.foil === l.foil);
	const drawerCards = $derived(
		(drawer === 'mine' ? data.mine.cards : data.theirs.cards)
			.map((o) => ({ o, c: details[o.card] }))
			.filter((x) => x.c)
			.toSorted((a, b) => a.c.title.localeCompare(b.c.title))
	);
</script>

<svelte:head><title>{i18n.t('trade.title', { name: partner })} · AnyCardz</title></svelte:head>

{#snippet side(which: 'mine' | 'theirs')}
	{@const s = trade[which]}
	<div class="tside {which}" class:sealed={s.sealed}>
		<header>
			<span class="avatar" class:you={which === 'mine'}
				>{which === 'mine' ? session.username[0]?.toUpperCase() : partner[0]?.toUpperCase()}</span
			>
			<b
				>{which === 'mine'
					? i18n.t('trade.you_give')
					: i18n.t('trade.they_give', { name: partner })}</b
			>
			<small>{i18n.t('trade.cards', { n: count(s.cards) })}</small>
		</header>
		<div class="tcards">
			{#each s.cards as l (`${l.card}|${l.foil}`)}
				{#each Array(l.count) as _, k (k)}
					<button
						type="button"
						class="tcard"
						class:changed={isAdded(which, l)}
						data-tag={i18n.t('trade.added')}
						onclick={() => remove(which, l)}
						disabled={!open || busy}
						aria-label={i18n.t('trade.remove', { name: details[l.card]?.title ?? l.card })}
					>
						{#if details[l.card]}<Card
								card={details[l.card]}
								foil={l.foil ? foilOf((which === 'mine' ? mineOwned : theirsOwned)[l.card]) : null}
							/>{/if}
					</button>
				{/each}
			{/each}
			{#each changes?.[which].removed ?? [] as l (`removed-${l.card}|${l.foil}`)}
				<span
					class="tcard removed"
					aria-label="{i18n.t('trade.removed')}: {details[l.card]?.title}"
				>
					{#if details[l.card]}<Card
							card={details[l.card]}
							foil={l.foil ? foilOf((which === 'mine' ? mineOwned : theirsOwned)[l.card]) : null}
						/>{/if}
				</span>
			{/each}
			{#if !s.cards.length && !changes?.[which].removed.length}
				<p class="tempty">
					{which === 'mine'
						? i18n.t('trade.empty_mine')
						: i18n.t('trade.empty_theirs', { name: partner })}
				</p>
			{/if}
		</div>
		<label class="tcoins">
			<span class="coin">c</span>
			<span class="sr"
				>{which === 'mine'
					? i18n.t('trade.coins_mine')
					: i18n.t('trade.coins_theirs', { name: partner })}</span
			>
			<input
				id="coins-{which}"
				type="number"
				min="0"
				step="50"
				inputmode="numeric"
				value={s.coins}
				disabled={!open || busy}
				onchange={(e) => setCoins(which, e.currentTarget.value)}
			/>
		</label>
		{#if s.sealed || broken[which]}
			<div class="seal" class:breaking={broken[which]} aria-hidden="true">
				{#if which === 'mine'}{i18n.t('trade.sealed_by_you')}{:else}{i18n.t('trade.sealed_by', {
						name: partner
					})}{/if}
			</div>
		{/if}
	</div>
{/snippet}

<div class="screen">
	<a class="back" href="/trade">← {i18n.t('trade.back')}</a>
	<div class="who">
		<span class="avatar big">{partner[0]?.toUpperCase()}</span>
		<div>
			<p class="eyebrow">{i18n.t(`trade.eyebrow_${trade.state}`)}</p>
			<h1>
				<a class="partner" href="/players/{encodeURIComponent(partner)}"
					>{i18n.t('trade.title', { name: partner })}</a
				>
			</h1>
		</div>
	</div>
	<p class="lead">
		{trade.state === 'done'
			? i18n.t('trade.done_lead')
			: trade.state === 'cancelled'
				? i18n.t('trade.cancelled_lead')
				: i18n.t('trades.lead')}
	</p>

	<div class="table" class:swapping class:closed={!open}>
		{@render side('mine')}
		<span class="swap-mid" aria-hidden="true"
			><svg viewBox="0 0 24 24"><path d="M4 8h14l-3-3M20 16H6l3 3" /></svg></span
		>
		{@render side('theirs')}
	</div>

	{#if open}
		<div class="bar">
			{#if !trade.mine.sealed}
				{#key trade.version}
					<SlideConfirm
						label={trade.theirs.sealed ? i18n.t('trade.seal_swap') : i18n.t('trade.seal')}
						onConfirm={seal}
					/>
				{/key}
			{:else}
				<p class="waiting" role="status">
					<span class="typing"><i></i><i></i><i></i></span>{i18n.t('trade.waiting', {
						name: partner
					})}
				</p>
			{/if}
			<button type="button" class="linkish" onclick={cancel}>{i18n.t('trade.cancel')}</button>
		</div>

		<section class="drawer">
			<div class="drawer-head">
				<div class="seg" role="group" aria-label={i18n.t('trade.drawer_label')}>
					<button
						type="button"
						aria-pressed={drawer === 'theirs'}
						onclick={() => (drawer = 'theirs')}
						>{i18n.t('trade.their_cards', { name: partner })}</button
					>
					<button type="button" aria-pressed={drawer === 'mine'} onclick={() => (drawer = 'mine')}
						>{i18n.t('trade.your_cards')}</button
					>
				</div>
				<p>{drawer === 'mine' ? i18n.t('trade.drawer_mine') : i18n.t('trade.drawer_theirs')}</p>
			</div>
			<div class="tray">
				{#each drawerCards as { o, c } (o.card)}
					{@const f = free(
						drawer === 'mine' ? mineOwned : theirsOwned,
						trade[drawer].cards,
						o.card
					)}
					{@const lacking = drawer === 'theirs' && !mineOwned[o.card]}
					<button
						type="button"
						class="tcard"
						onclick={() => add(drawer, o.card)}
						disabled={busy || f.plain + f.foil < 1}
						aria-label={c.title}
					>
						<Card
							card={c}
							foil={f.plain < 1 && f.foil > 0
								? foilOf((drawer === 'mine' ? mineOwned : theirsOwned)[o.card])
								: null}
						>
							{#snippet badge()}<span class="c-badge" class:lacking>×{f.plain + f.foil}</span
								>{/snippet}
						</Card>
					</button>
				{:else}
					<p class="emptynote">{i18n.t('trade.no_cards')}</p>
				{/each}
			</div>
		</section>
	{/if}
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 16px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 1000px;
		margin: 0 auto;
	}
	@media (min-width: 700px) {
		.screen {
			padding: 24px;
		}
	}
	.partner {
		color: inherit;
		text-decoration: none;
	}
	.partner:hover {
		text-decoration: underline;
	}
	.back {
		color: var(--muted);
		font-size: 13px;
		text-decoration: none;
	}
	.who {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	h1 {
		font-size: 26px;
	}
	.lead {
		color: var(--muted);
		max-width: 62ch;
	}
	.avatar {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: var(--rare);
		color: #fff;
		font: 800 12px var(--display);
		flex: none;
	}
	.avatar.you {
		background: var(--accent);
		color: var(--accent-ink);
	}
	.avatar.big {
		width: 46px;
		height: 46px;
		font-size: 18px;
	}
	.table {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		gap: 10px;
		position: relative;
	}
	.tside {
		position: relative;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 18px;
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-height: 220px;
		min-width: 0;
		transition: box-shadow 0.3s;
	}
	.tside.sealed {
		box-shadow: inset 0 0 0 2px color-mix(in srgb, #c0332b 55%, transparent);
	}
	.tside header {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 700;
		min-width: 0;
	}
	.tside header b {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.tside header small {
		margin-left: auto;
		font: 600 11px var(--mono);
		color: var(--muted);
		flex: none;
	}
	.tcards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(58px, 1fr));
		gap: 8px;
		align-content: start;
		flex: 1;
		transition:
			transform 0.7s cubic-bezier(0.6, 0, 0.3, 1),
			opacity 0.7s;
	}
	.swapping .mine .tcards {
		transform: translateX(calc(100% + 34px));
		opacity: 0.2;
	}
	.swapping .theirs .tcards {
		transform: translateX(calc(-100% - 34px));
		opacity: 0.2;
	}
	.tcard {
		position: relative;
		border: 0;
		background: none;
		padding: 0;
		display: block;
		transition: transform 0.2s;
	}
	.tcard:not(:disabled):hover {
		transform: translateY(-3px);
	}
	.tcard:disabled {
		cursor: default;
	}
	.tcard.changed {
		animation: changed 1s ease-in-out 3;
		border-radius: 6px;
	}
	.tcard.changed::after {
		content: attr(data-tag);
		position: absolute;
		left: 50%;
		bottom: -8px;
		translate: -50% 0;
		font: 700 9px var(--mono);
		text-transform: uppercase;
		white-space: nowrap;
		background: var(--warn);
		color: #111;
		padding: 2px 5px;
		border-radius: 4px;
	}
	@keyframes changed {
		50% {
			box-shadow: 0 0 0 4px var(--warn);
		}
	}
	.tcard.removed {
		opacity: 0.5;
		filter: grayscale(1);
	}
	.tcard.removed::after {
		content: '✕';
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		font: 800 26px var(--display);
		color: var(--danger);
	}
	.tempty {
		color: var(--muted);
		font-size: 12.5px;
		text-align: center;
		padding: 18px 6px;
		border: 1.5px dashed var(--line);
		border-radius: 12px;
		grid-column: 1 / -1;
	}
	.tcoins {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.tcoins .coin {
		width: 18px;
		height: 18px;
		font-size: 9px;
	}
	.tcoins input {
		width: 100%;
		min-width: 0;
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 8px;
		padding: 6px 8px;
		font-family: var(--mono);
	}
	.seal {
		position: absolute;
		right: 10px;
		bottom: 46px;
		width: 84px;
		height: 84px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: radial-gradient(circle at 40% 35%, #e0574f, #a3221c 70%);
		box-shadow:
			0 4px 10px rgba(80, 0, 0, 0.4),
			inset 0 0 0 5px rgba(0, 0, 0, 0.12),
			inset 0 0 0 7px rgba(255, 255, 255, 0.12);
		color: #ffd7d2;
		font: 800 11px/1.05 var(--display);
		text-align: center;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		padding: 10px;
		rotate: -14deg;
		animation: stamp 0.45s cubic-bezier(0.3, 1.6, 0.5, 1);
		pointer-events: none;
		z-index: 2;
	}
	.seal.breaking {
		animation: crack 0.6s forwards;
	}
	@keyframes crack {
		30% {
			scale: 1.1;
		}
		100% {
			scale: 0.6;
			opacity: 0;
			rotate: 20deg;
			filter: brightness(0.5);
		}
	}
	.swap-mid {
		position: absolute;
		left: 50%;
		top: 50%;
		translate: -50% -50%;
		width: 34px;
		height: 34px;
		border-radius: 50%;
		background: var(--ink);
		color: var(--bg);
		display: grid;
		place-items: center;
		z-index: 3;
		box-shadow: 0 0 0 4px var(--bg);
	}
	.swap-mid svg {
		width: 18px;
		height: 18px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.closed .tside {
		opacity: 0.85;
	}
	.bar {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.bar .linkish {
		align-self: center;
	}
	.waiting {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--muted);
		font-size: 13.5px;
		padding: 14px;
		border-radius: 12px;
		background: var(--surface-2);
	}
	.typing {
		display: inline-flex;
		gap: 3px;
	}
	.typing i {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--muted);
		animation: dot 1s infinite;
	}
	.typing i:nth-child(2) {
		animation-delay: 0.15s;
	}
	.typing i:nth-child(3) {
		animation-delay: 0.3s;
	}
	@keyframes dot {
		50% {
			transform: translateY(-4px);
			opacity: 0.4;
		}
	}
	.drawer {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.drawer-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.drawer-head p {
		font-size: 12.5px;
		color: var(--muted);
	}
	.tray {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(78px, 1fr));
		gap: 12px;
		padding: 4px 2px;
	}
	.tray .tcard:disabled {
		opacity: 0.35;
	}
	.c-badge.lacking {
		background: var(--ok);
		color: #fff;
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
