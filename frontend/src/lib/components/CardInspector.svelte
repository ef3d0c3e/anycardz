<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts, getCatalog } from '#lib/session.svelte.ts';
	import { post, artUrl } from '#lib/api.ts';
	import { flyCoins, pad3 } from '#lib/fx.ts';
	import type { CardDto, OwnedDto } from '#lib/types.ts';
	import Card from './Card.svelte';
	import SlideConfirm from './SlideConfirm.svelte';

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
				<Card {card} foil={foils ? 'foil3' : null} tilt large />
			{:else}
				<div class="card ghost r-tag-{card.rarity}">
					<img src={artUrl(card.slug)} alt="" />
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
			{:else}
				<p class="muted">{i18n.t('inspect.pull_hint')}</p>
			{/if}
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
