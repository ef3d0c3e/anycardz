<script lang="ts">
	import { getToasts } from '#lib/session.svelte.ts';
	const toasts = getToasts();
</script>

<div class="toasts" aria-live="polite">
	{#each toasts.list as t (t.id)}
		<div class="toast {t.kind}">{t.text}</div>
	{/each}
</div>

<style>
	/* Phones: over the header, clear of the controls at the top of the screens */
	.toasts {
		position: fixed;
		top: calc(8px + env(safe-area-inset-top, 0px));
		left: 50%;
		translate: -50% 0;
		z-index: 80;
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: center;
		pointer-events: none;
		width: min(420px, calc(100% - 24px));
	}
	/* Wider screens: the bottom left of the main column, away from the centred controls */
	@media (min-width: 700px) {
		.toasts {
			top: auto;
			bottom: 24px;
			left: calc(86px + 16px);
			translate: none;
			align-items: flex-start;
			width: min(360px, calc(100% - 120px));
		}
	}
	.toast {
		background: var(--ink);
		color: var(--bg);
		padding: 10px 14px;
		border-radius: 12px;
		font-size: 13.5px;
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
		animation: rise 0.3s cubic-bezier(0.2, 1.3, 0.4, 1);
		text-align: center;
	}
	@media (max-width: 699px) {
		.toast {
			animation-name: drop;
		}
	}
	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(12px);
		}
	}
	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-12px);
		}
	}
	.toast.warn {
		background: var(--warn);
		color: #1b1200;
	}
</style>
