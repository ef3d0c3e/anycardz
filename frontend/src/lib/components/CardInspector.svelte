<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts, getCatalog } from '#lib/session.svelte.ts';
	import { api, post, artUrl } from '#lib/api.ts';
	import { bounds, FEE_PERCENT, net } from '#lib/market.ts';
	import { flyCoins, pad3 } from '#lib/fx.ts';
	import type { CardDto, MarketPage, OwnedDto } from '#lib/types.ts';
	import Card from './Card.svelte';
	import { foilOf } from '#lib/foil.ts';
	import SlideConfirm from './SlideConfirm.svelte';
	import { getLive } from '#lib/live.svelte.ts';
	import { brag } from '#lib/brag.ts';

	interface Props {
		card: CardDto;
		owned: OwnedDto | undefined;
		onClose: () => void;
		/** After a sale, with the copies left */
		onSold: (left: { count: number; foils: number }) => void;
	}
	let { card, owned, onClose, onSold }: Props = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	const catalog = getCatalog();
	const live = getLive();

	const PRICE = { common: 10, rare: 50, legendary: 250 } as const;
	const RATE = { common: '80%', rare: '15%', legendary: '5%' } as const;
	const count = $derived(owned?.count ?? 0);
	const foils = $derived(owned?.foils ?? 0);
	// Plain copies sell first: foils are worth keeping
	const sellFoil = $derived(count > 0 && count === foils);
	const price = $derived(PRICE[card.rarity] * (sellFoil ? 2 : 1));
	let cardEl = $state<HTMLElement>();
	let dialog: HTMLDivElement;

	$effect(() => dialog?.focus());

	// ---- the market: what others ask, and listing a copy ----
	let offers = $state<{ total: number; from: number } | null>(null);
	$effect(() => {
		const slug = card.slug;
		api<MarketPage>(`/market?card=${encodeURIComponent(slug)}&sort=price`)
			.then((m) => (offers = { total: m.total, from: m.items[0]?.price ?? 0 }))
			.catch(() => (offers = null));
	});
	let listing = $state(false);
	let listFoil = $state(false);
	let askPrice = $state(0);
	const plain = $derived(count - foils);
	const range = $derived(bounds(card.rarity, listFoil));
	const validPrice = $derived(
		Number.isInteger(askPrice) && askPrice >= range.min && askPrice <= range.max
	);
	function startListing() {
		listFoil = plain < 1;
		const b = bounds(card.rarity, listFoil);
		// What others ask is a fair start, within the bounds
		askPrice = Math.min(b.max, Math.max(b.min, offers?.from || b.min * 2));
		listing = true;
	}
	function pickFoil(f: boolean) {
		listFoil = f;
		const b = bounds(card.rarity, f);
		askPrice = Math.min(b.max, Math.max(b.min, askPrice));
	}
	async function list(e: SubmitEvent) {
		e.preventDefault();
		if (!validPrice) return;
		try {
			const res = await post<{ remaining: number }>('/market', {
				card: card.slug,
				foil: listFoil,
				price: askPrice
			});
			toasts.show(i18n.t('market.listed', { name: card.title, n: askPrice }));
			listing = false;
			onSold({ count: res.remaining, foils: listFoil ? foils - 1 : foils });
		} catch (err) {
			toasts.show(i18n.error(err), 'warn');
		}
	}

	async function sell() {
		try {
			const res = await post<{ coins: number; sold_for: number; remaining: number }>(
				'/binder/sell',
				{ card: card.slug, foil: sellFoil }
			);
			if (cardEl) await flyCoins(cardEl);
			session.coins = res.coins;
			toasts.show(i18n.t('inspect.sold', { name: card.title, n: res.sold_for }));
			onSold({ count: res.remaining, foils: sellFoil ? foils - 1 : foils });
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="modal" onclick={(e) => e.target === e.currentTarget && onClose()}>
	<div
		class="sheet"
		role="dialog"
		aria-modal="true"
		aria-label={count ? card.title : i18n.t('inspect.unknown')}
		tabindex="-1"
		bind:this={dialog}
	>
		<button type="button" class="x" onclick={onClose} aria-label={i18n.t('common.close')}>✕</button>
		<div class="ins-card" bind:this={cardEl}>
			{#if count}
				<Card {card} foil={foilOf(owned)} tilt large />
			{:else}
				<div class="card ghost r-tag-{card.rarity}">
					<img src={artUrl(card.slug, 96)} alt="" />
					<span class="g-r"></span><span class="g-no">#{pad3(card.number)}</span><span class="g-q"
						>{i18n.t('binder.not_found')}</span
					>
				</div>
			{/if}
		</div>
		<div class="info">
			<p class="eyebrow">
				{catalog?.get(card.collection)?.title ?? card.collection} · #{pad3(card.number)}
			</p>
			<h2>{count ? card.title : i18n.t('inspect.unknown')}</h2>
			<span class="rchip r-tag-{card.rarity}">{i18n.t(`card.${card.rarity}`)}</span>
			{#if count && card.description}<p class="desc">{card.description}</p>{/if}
			<dl class="facts">
				<dt>{i18n.t('inspect.owned')}</dt>
				<dd>
					{count ? i18n.t('inspect.copies', { n: count }) : i18n.t('inspect.none')}{foils
						? ` · ${i18n.t('inspect.foils', { n: foils })}`
						: ''}
				</dd>
				<dt>{i18n.t('inspect.sells_for')}</dt>
				<dd class="mono">
					{i18n.t('inspect.sell_value', {
						plain: PRICE[card.rarity],
						foil: PRICE[card.rarity] * 2
					})}
				</dd>
				<dt>{i18n.t('inspect.drop_rate')}</dt>
				<dd>{i18n.t('inspect.drop_rate_value', { p: RATE[card.rarity] })}</dd>
			</dl>
			{#if count}
				{#key count}
					<SlideConfirm
						tone="legend"
						label={i18n.t(sellFoil ? 'inspect.sell_foil' : 'inspect.sell', { n: price })}
						onConfirm={sell}
					/>
				{/key}
				<button
					type="button"
					class="btn sm brag"
					onclick={() => brag(card.slug, foils > 0, { i18n, live, toasts })}
				>
					{i18n.t('live.brag', { name: card.title })}
				</button>
			{:else}
				<p class="muted">{i18n.t('inspect.pull_hint')}</p>
			{/if}
			<section class="market" aria-labelledby="ins-market">
				<h3 id="ins-market">{i18n.t('market.inspect_title')}</h3>
				{#if offers}
					<p class="muted">
						{offers.total
							? i18n.t('market.inspect_from', { n: offers.total, price: i18n.num(offers.from) })
							: i18n.t('market.inspect_none')}
						{#if offers.total}<a
								href="/market?card={encodeURIComponent(card.slug)}"
								onclick={onClose}>{i18n.t('market.inspect_see')}</a
							>{/if}
					</p>
				{/if}
				{#if count && !listing}
					<button type="button" class="btn sm" onclick={startListing}
						>{i18n.t('market.list_open')}</button
					>
				{:else if count}
					<form class="list-form" onsubmit={list}>
						{#if plain > 0 && foils > 0}
							<div class="seg" role="group" aria-label={i18n.t('market.copy')}>
								<button type="button" aria-pressed={!listFoil} onclick={() => pickFoil(false)}
									>{i18n.t('market.copy_plain')}</button
								>
								<button type="button" aria-pressed={listFoil} onclick={() => pickFoil(true)}
									>{i18n.t('market.copy_foil')}</button
								>
							</div>
						{/if}
						<label class="price-field">
							<span>{i18n.t('market.price')}</span>
							<input
								type="number"
								inputmode="numeric"
								min={range.min}
								max={range.max}
								step="1"
								bind:value={askPrice}
								aria-describedby="ins-price-hint"
							/>
						</label>
						<p class="muted" id="ins-price-hint">
							{i18n.t('market.price_hint', { min: range.min, max: i18n.num(range.max) })} ·
							{validPrice
								? i18n.t('market.you_get', { n: i18n.num(net(askPrice)), p: FEE_PERCENT })
								: ''}
						</p>
						<div class="list-actions">
							<button type="submit" class="btn" disabled={!validPrice}
								>{i18n.t('market.list_submit', { n: i18n.num(validPrice ? askPrice : 0) })}</button
							>
							<button type="button" class="btn sm ghost-btn" onclick={() => (listing = false)}
								>{i18n.t('market.cancel')}</button
							>
						</div>
						<p class="muted small">{i18n.t('market.locked_hint')}</p>
					</form>
				{/if}
			</section>
		</div>
	</div>
</div>

<style>
	.modal {
		position: fixed;
		inset: 0;
		z-index: 60;
		display: grid;
		place-items: center;
		background: rgba(8, 10, 25, 0.6);
		backdrop-filter: blur(6px);
		padding: 16px;
		animation: fade 0.2s;
	}
	.sheet {
		position: relative;
		background: var(--surface);
		border-radius: 24px;
		padding: 20px;
		width: min(720px, 100%);
		max-height: 100%;
		overflow: auto;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 18px;
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
		animation: up 0.35s cubic-bezier(0.2, 1.2, 0.4, 1);
		outline: none;
	}
	@media (min-width: 640px) {
		.sheet {
			grid-template-columns: 260px minmax(0, 1fr);
			padding: 26px;
		}
	}
	.ins-card {
		width: min(240px, 62vw);
		margin: 0 auto;
	}
	.info {
		display: flex;
		flex-direction: column;
		gap: 12px;
		min-width: 0;
	}
	h2 {
		font-size: 30px;
	}
	.desc,
	.muted {
		color: var(--muted);
		font-size: 13.5px;
		max-width: 60ch;
	}
	.facts {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 6px 14px;
		font-size: 13.5px;
		margin: 0;
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
		font-weight: 600;
	}
	.rchip {
		display: inline-flex;
		font: 700 11px var(--mono);
		text-transform: uppercase;
		letter-spacing: 0.08em;
		padding: 4px 8px;
		border-radius: 6px;
		background: color-mix(in srgb, var(--rc) 18%, transparent);
		color: var(--rc);
		align-self: flex-start;
	}
	.brag {
		align-self: flex-start;
	}
	.market {
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: flex-start;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}
	.market a {
		color: var(--accent);
		font-weight: 600;
	}
	.market h3 {
		font-size: 14px;
	}
	.list-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-self: stretch;
	}
	.list-form .seg {
		align-self: flex-start;
	}
	.price-field {
		display: flex;
		align-items: center;
		gap: 10px;
		font-weight: 600;
		font-size: 13.5px;
	}
	.price-field input {
		width: 120px;
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 10px;
		padding: 8px 10px;
		font: 700 16px var(--mono);
	}
	.list-actions {
		display: flex;
		gap: 8px;
		align-items: center;
		flex-wrap: wrap;
	}
	.small {
		font-size: 12px;
	}
	.x {
		position: absolute;
		top: 10px;
		right: 10px;
		width: 36px;
		height: 36px;
		border-radius: 10px;
		border: 0;
		background: var(--surface-2);
		font-size: 18px;
		z-index: 2;
	}
	.ghost {
		overflow: hidden;
		background: var(--surface);
		box-shadow: inset 0 0 0 1.5px var(--line);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 4px;
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
</style>
