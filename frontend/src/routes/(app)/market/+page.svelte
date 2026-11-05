<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { api, ApiError, post } from '#lib/api.ts';
	import { tone } from '#lib/fx.ts';
	import { fee } from '#lib/market.ts';
	import { getTopup } from '#lib/topup.svelte.ts';
	import type { ListingDto, MarketPage, Rarity } from '#lib/types.ts';
	import Card from '#lib/components/Card.svelte';
	import Avatar from '#lib/components/Avatar.svelte';
	import HoldButton from '#lib/components/HoldButton.svelte';
	import TradeTabs from '#lib/components/TradeTabs.svelte';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	const topup = getTopup();

	// ---- filters, kept in the URL ----
	// svelte-ignore state_referenced_locally
	let q = $state(data.filters.q ?? '');
	const rarity = $derived((data.filters.rarity ?? '') as Rarity | '');
	const foilOnly = $derived(data.filters.foil === 'true');
	const sort = $derived(data.filters.sort ?? 'price');
	function apply(changes: Record<string, string | null>) {
		const p = new URLSearchParams(data.filters);
		for (const [k, v] of Object.entries(changes)) {
			if (v) p.set(k, v);
			else p.delete(k);
		}
		const s = p.toString();
		goto(s ? `/market?${s}` : '/market', { replace: true, reset: false });
	}
	let timer: ReturnType<typeof setTimeout>;
	function typed() {
		clearTimeout(timer);
		timer = setTimeout(() => apply({ q: q.trim() || null, card: null }), 250);
	}

	// ---- listings, a page at a time ----
	let more = $state<ListingDto[]>([]);
	let pageNo = $state(0);
	let bought = $state<number[]>([]);
	$effect(() => {
		// New filters: start over
		void data.market;
		more = [];
		pageNo = 0;
		bought = [];
	});
	const items = $derived([...data.market.items, ...more].filter((l) => !bought.includes(l.id)));
	const hasMore = $derived(
		data.market.items.length + more.length < data.market.total && items.length > 0
	);
	async function loadMore() {
		const p = new URLSearchParams(data.filters);
		p.set('page', String(pageNo + 1));
		const next = await api<MarketPage>(`/market?${p}`).catch(() => null);
		if (!next) return;
		pageNo = next.page;
		more = [...more, ...next.items];
	}

	function askTopup(l: ListingDto) {
		topup.ask({
			need: l.price - session.coins,
			reason: i18n.t('market.topup_reason', { name: l.card.title }),
			then: () => buy(l)
		});
	}
	async function buy(l: ListingDto) {
		if (session.coins < l.price) return askTopup(l);
		try {
			const res = await post<{ coins: number }>(`/market/${l.id}/buy`);
			session.coins = res.coins;
			bought = [...bought, l.id];
			tone(780, 0.1, 'triangle', 0.05);
			toasts.show(i18n.t('market.bought', { name: l.card.title }));
		} catch (e) {
			if (e instanceof ApiError && e.code === 'not_enough_coins') return askTopup(l);
			if (e instanceof ApiError && e.code === 'listing_gone') bought = [...bought, l.id];
			toasts.show(i18n.error(e), 'warn');
		}
	}

	// ---- the player's own listings ----
	async function withdraw(l: ListingDto) {
		try {
			await api(`/market/${l.id}`, { method: 'DELETE' });
			toasts.show(i18n.t('market.withdrawn', { name: l.card.title }));
			await invalidate('app:market');
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
	const RARITIES: Rarity[] = ['common', 'rare', 'legendary'];
</script>

<svelte:head><title>{i18n.t('market.title')} · AnyCardz</title></svelte:head>

<div class="screen">
	<TradeTabs />
	<div>
		<h1>{i18n.t('market.title')}</h1>
		<p class="lead">{i18n.t('market.lead')}</p>
	</div>

	<section aria-label={i18n.t('market.title')}>
		<div class="filters">
			<input
				type="search"
				bind:value={q}
				oninput={typed}
				placeholder={i18n.t('market.search')}
				aria-label={i18n.t('market.search')}
				autocomplete="off"
			/>
			<div class="chips" role="group" aria-label={i18n.t('market.rarity')}>
				<button
					type="button"
					class="chip"
					aria-pressed={rarity === ''}
					onclick={() => apply({ rarity: null })}>{i18n.t('market.all_rarities')}</button
				>
				{#each RARITIES as r (r)}
					<button
						type="button"
						class="chip r-tag-{r}"
						aria-pressed={rarity === r}
						onclick={() => apply({ rarity: r })}>{i18n.t(`card.${r}`)}</button
					>
				{/each}
				<button
					type="button"
					class="chip foil"
					aria-pressed={foilOnly}
					onclick={() => apply({ foil: foilOnly ? null : 'true' })}
					>{i18n.t('market.foil_only')}</button
				>
			</div>
			<label class="sort">
				<span>{i18n.t('market.sort')}</span>
				<select value={sort} onchange={(e) => apply({ sort: e.currentTarget.value })}>
					<option value="price">{i18n.t('market.sort_price')}</option>
					<option value="new">{i18n.t('market.sort_new')}</option>
				</select>
			</label>
		</div>
		<p class="count" aria-live="polite">{i18n.t('market.count', { n: data.market.total })}</p>

		{#if items.length}
			<ul class="grid">
				{#each items as l (l.id)}
					{@const short = session.coins < l.price}
					<li class="listing">
						<Card card={l.card} foil={l.foil} />
						<a class="seller" href="/players/{encodeURIComponent(l.seller)}">
							<Avatar name={l.seller} src={l.seller_avatar} size={20} />
							<span>{i18n.t('market.by', { name: l.seller })}</span>
						</a>
						<HoldButton
							class="btn buy {short ? 'short' : ''}"
							label={i18n.t('market.buy_label', { name: l.card.title, n: l.price })}
							ms={short ? 350 : 650}
							onDone={() => buy(l)}
							onTooShort={() => (short ? askTopup(l) : toasts.show(i18n.t('lobby.keep_holding')))}
						>
							<span class="price"><span class="coin">c</span>{i18n.num(l.price)}</span>
							<span class="verb">{short ? i18n.t('lobby.topup_short') : i18n.t('lobby.hold')}</span>
						</HoldButton>
					</li>
				{/each}
			</ul>
			{#if hasMore}
				<button type="button" class="btn more" onclick={loadMore}>{i18n.t('market.more')}</button>
			{/if}
		{:else}
			<p class="empty">
				{Object.keys(data.filters).some((k) => k !== 'sort')
					? i18n.t('market.none')
					: i18n.t('market.empty')}
			</p>
		{/if}
	</section>

	<section class="panel" id="selling" aria-labelledby="selling-title">
		<div class="sec-head">
			<h2 id="selling-title">{i18n.t('market.selling')}</h2>
			<span class="muted"
				>{i18n.t('market.selling_count', {
					n: data.mine.open.length,
					max: data.mine.max_listings
				})}</span
			>
		</div>
		{#if data.mine.open.length}
			<ul class="mine">
				{#each data.mine.open as l (l.id)}
					<li>
						<div class="mini"><Card card={l.card} foil={l.foil} /></div>
						<span class="what">
							<b>{l.card.title}</b>
							<small
								>{i18n.t('market.you_get', {
									n: i18n.num(l.price - fee(l.price)),
									p: data.mine.fee_percent
								})}</small
							>
						</span>
						<span class="price"><span class="coin">c</span>{i18n.num(l.price)}</span>
						<button
							type="button"
							class="btn sm"
							aria-label={i18n.t('market.withdraw_label', { name: l.card.title })}
							onclick={() => withdraw(l)}>{i18n.t('market.withdraw')}</button
						>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">{i18n.t('market.selling_none')}</p>
		{/if}

		{#if data.mine.sold.length}
			<h3>{i18n.t('market.sold_title')}</h3>
			<ul class="mine sold">
				{#each data.mine.sold as s (s.id)}
					<li>
						<div class="mini"><Card card={s.card} foil={s.foil} /></div>
						<span class="what">
							<b>{s.card.title}</b>
							<small
								>{i18n.t('market.sold_line', {
									name: s.buyer ?? i18n.t('market.gone_player'),
									price: i18n.num(s.price),
									fee: i18n.num(s.fee)
								})}</small
							>
						</span>
						<span class="price plus">+{i18n.num(s.price - s.fee)}</span>
					</li>
				{/each}
			</ul>
		{/if}
	</section>
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 18px;
		padding: 16px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 1100px;
		margin: 0 auto;
	}
	@media (min-width: 700px) {
		.screen {
			padding: 24px;
		}
	}
	h1 {
		font-size: 28px;
	}
	h2 {
		font-size: 17px;
	}
	h3 {
		font-size: 14px;
		margin: 18px 0 8px;
	}
	.lead,
	.muted {
		color: var(--muted);
		max-width: 60ch;
		margin-top: 6px;
	}
	.filters {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
		align-items: center;
	}
	input[type='search'] {
		flex: 1 1 240px;
		min-width: 0;
		border: 1px solid var(--line);
		background: var(--surface);
		border-radius: 12px;
		padding: 11px 14px;
		font-size: 16px;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.chip {
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		border-radius: 999px;
		padding: 6px 12px;
		font-size: 13px;
		font-weight: 600;
	}
	.chip[class*='r-tag-'] {
		box-shadow: inset 3px 0 0 var(--rc);
	}
	.chip[aria-pressed='true'] {
		background: var(--ink);
		color: var(--bg);
		border-color: var(--ink);
	}
	.chip.foil[aria-pressed='true'] {
		background: linear-gradient(110deg, #ff5fa8, #ffc24d, #3ee6b5, #5aa8ff);
		color: #111;
		border-color: transparent;
	}
	.sort {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--muted);
	}
	.sort select {
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		border-radius: 10px;
		padding: 6px 8px;
		font-size: 14px;
	}
	.count {
		margin-top: 10px;
		font-size: 13px;
		color: var(--muted);
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.grid {
		margin-top: 10px;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 18px 14px;
	}
	.listing {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-width: 0;
	}
	.seller {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 12.5px;
		color: var(--muted);
		text-decoration: none;
		min-width: 0;
	}
	.seller span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.listing :global(.buy) {
		width: 100%;
		padding: 9px 10px;
		font-size: 13px;
		justify-content: space-between;
	}
	.listing :global(.buy.short) {
		--fill: var(--legend);
		border-style: dashed;
	}
	.price {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-family: var(--mono);
		font-weight: 700;
	}
	.price .coin {
		width: 15px;
		height: 15px;
		font-size: 8px;
	}
	.price.plus {
		color: var(--ok);
	}
	.more {
		align-self: center;
		margin: 16px auto 0;
		display: block;
	}
	.empty {
		margin-top: 12px;
		text-align: center;
		padding: 24px;
		border: 1.5px dashed var(--line);
		border-radius: 14px;
		color: var(--muted);
	}
	.panel {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 18px;
		padding: 16px;
	}
	.sec-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 10px;
		flex-wrap: wrap;
	}
	.sec-head .muted {
		margin: 0;
		font-size: 13px;
	}
	.mine {
		margin-top: 10px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.mine li {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 10px;
		border: 1px solid var(--line);
		border-radius: 14px;
	}
	.mini {
		width: 38px;
		flex: none;
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.what b,
	.what small {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.what small {
		color: var(--muted);
	}
	.sold li {
		opacity: 0.85;
	}
</style>
