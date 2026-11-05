<script lang="ts">
	import type { Snippet } from 'svelte';
	import { artSrcset, artUrl, assetUrl } from '#lib/api.ts';
	import { tilt as tiltAction } from '#lib/actions/tilt.ts';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog } from '#lib/session.svelte.ts';
	import { pad3 } from '#lib/fx.ts';
	import type { CardDto } from '#lib/types.ts';

	interface Props {
		card: CardDto;
		/** Foil pattern (foil1..foil6) */
		foil?: string | null;
		tilt?: boolean;
		/** Load the 840px artwork: big displays */
		large?: boolean;
		/** Load the artwork now: cards that appear in motion */
		eager?: boolean;
		class?: string;
		badge?: Snippet;
	}
	let {
		card,
		foil = null,
		tilt = false,
		large = false,
		eager = false,
		class: cls = '',
		badge
	}: Props = $props();
	const i18n = getI18n();
	const catalog = getCatalog();
	const setName = $derived(catalog?.get(card.collection)?.title ?? card.collection);
</script>

<!-- The face tilts; badges sit on the card around it, so a tilt never clips them -->
<div
	class="card r-{card.rarity} {cls}"
	style:--cp={card.primary}
	style:--cs={card.secondary}
	style:--foil-mask={foil ? `url(${assetUrl(`${foil}.png`)})` : undefined}
>
	<div class="face" class:tilt use:tiltAction={tilt}>
		<div class="cf">
			<div class="c-top"><span>{setName}</span><span>{pad3(card.number)}/{card.total}</span></div>
			<div class="c-art">
				<!-- A fixed hint: cards in grids are about 160px wide, a large one about 300px.
				     ("auto" re-picks the biggest file for images leaving the page in Chrome) -->
				<img
					src={artUrl(card.slug, large ? 840 : 420)}
					srcset={artSrcset(card.slug, large)}
					sizes={large ? '300px' : '160px'}
					alt=""
					draggable="false"
					loading={eager ? 'eager' : 'lazy'}
				/>
			</div>
			<div class="c-name">{card.title}</div>
			<p class="c-desc">{card.description}</p>
			<div class="c-foot">
				<span class="gem"></span>{i18n.t(`card.${card.rarity}`)}{foil
					? ` · ${i18n.t('card.foil')}`
					: ''}
			</div>
		</div>
		{#if foil}<div class="holo"></div>{/if}
		<div class="glare"></div>
	</div>
	{@render badge?.()}
</div>
