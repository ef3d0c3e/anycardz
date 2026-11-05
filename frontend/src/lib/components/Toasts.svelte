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
	.toasts {
		position: fixed;
		/* Under the header, clear of the buttons at the bottom of the screens */
		top: calc(64px + env(safe-area-inset-top, 0px));
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
	@media (min-width: 700px) {
		.toasts {
			left: auto;
			right: 16px;
			translate: none;
			align-items: flex-end;
			width: min(360px, calc(100% - 32px));
		}
	}
	/* The live panel holds the right column: the toasts stay in the main one */
	@media (min-width: 1080px) {
		.toasts {
			right: calc(320px + 16px);
		}
	}
	.toast {
		background: var(--ink);
		color: var(--bg);
		padding: 10px 14px;
		border-radius: 12px;
		font-size: 13.5px;
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
		animation: drop 0.3s cubic-bezier(0.2, 1.3, 0.4, 1);
		text-align: center;
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
