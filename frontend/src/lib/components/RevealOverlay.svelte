<script lang="ts">
	import { onMount } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { burst } from '#lib/fx.ts';
	import type { CardDto } from '#lib/types.ts';
	import Card from './Card.svelte';

	/** A won card, full screen, like a pull from a pack. Tap anywhere to go on. */
	interface Props {
		card: CardDto;
		foil?: string | null;
		eyebrow: string;
		/** One line under the card name */
		detail?: string;
		onClose: () => void;
	}
	let { card, foil = null, eyebrow, detail, onClose }: Props = $props();
	const i18n = getI18n();
	let stage = $state<HTMLDivElement>();
	let button = $state<HTMLButtonElement>();

	onMount(() => {
		button?.focus();
		const color = foil ? '#ffcf5c' : card.rarity === 'legendary' ? '#ffcf5c' : '#41e4cf';
		const t = setTimeout(() => stage && burst(stage, color, 48), 250);
		return () => clearTimeout(t);
	});
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
	class="reveal"
	class:gold={foil || card.rarity === 'legendary'}
	role="dialog"
	tabindex="-1"
	aria-modal="true"
	aria-labelledby="reveal-title"
	onclick={onClose}
>
	<div class="stage" bind:this={stage}>
		<p class="eyebrow">{eyebrow}</p>
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
		<div class="big" onclick={(e) => e.stopPropagation()}>
			<Card {card} {foil} tilt large />
		</div>
		<h2 id="reveal-title">{card.title}</h2>
		{#if detail}<p class="detail">{detail}</p>{/if}
		<button type="button" class="btn" bind:this={button} onclick={onClose}
			>{i18n.t('reveal.continue')}</button
		>
	</div>
</div>

<style>
	.reveal {
		position: fixed;
		inset: 0;
		z-index: 75;
		display: grid;
		place-items: center;
		overflow: hidden;
		padding: calc(16px + env(safe-area-inset-top, 0px)) max(16px, env(safe-area-inset-right, 0px))
			calc(16px + env(safe-area-inset-bottom, 0px)) max(16px, env(safe-area-inset-left, 0px));
		background: radial-gradient(ellipse at 50% 40%, var(--felt-hi), var(--felt) 70%);
		color: var(--felt-ink);
		animation: fade 0.3s;
		cursor: pointer;
	}
	/* Light rays turning behind the card */
	.reveal::before {
		content: '';
		position: absolute;
		left: 50%;
		top: 42%;
		width: 200vmax;
		height: 200vmax;
		margin: -100vmax 0 0 -100vmax;
		background: repeating-conic-gradient(rgba(65, 228, 207, 0.18) 0 5deg, transparent 5deg 15deg);
		-webkit-mask: radial-gradient(circle, #000 0, transparent 32%);
		mask: radial-gradient(circle, #000 0, transparent 32%);
		animation: spin 18s linear infinite;
		pointer-events: none;
	}
	.reveal.gold::before {
		background: repeating-conic-gradient(rgba(255, 196, 80, 0.24) 0 5deg, transparent 5deg 15deg);
	}
	@keyframes spin {
		to {
			rotate: 360deg;
		}
	}
	.stage {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 14px;
		text-align: center;
	}
	.eyebrow {
		color: rgba(238, 240, 255, 0.7);
	}
	.big {
		text-align: left;
		width: min(300px, 72vw, calc((100dvh - 260px) * 63 / 88));
		cursor: default;
		animation: rise 0.7s cubic-bezier(0.2, 1.3, 0.4, 1);
	}
	@keyframes rise {
		from {
			transform: translateY(40px) scale(0.6) rotateY(160deg);
			opacity: 0;
		}
	}
	h2 {
		font-size: 28px;
	}
	.detail {
		color: rgba(238, 240, 255, 0.75);
		max-width: 40ch;
	}
	.btn {
		background: var(--felt-ink);
		color: var(--felt);
		border-color: transparent;
		min-width: 160px;
	}
</style>
