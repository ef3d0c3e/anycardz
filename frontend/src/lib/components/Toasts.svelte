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
		left: 50%;
		bottom: calc(96px + env(safe-area-inset-bottom, 0px));
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
			bottom: 24px;
		}
	}
	.toast {
		background: var(--ink);
		color: var(--bg);
		padding: 10px 14px;
		border-radius: 12px;
		font-size: 13.5px;
		box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.5);
		animation: up 0.3s cubic-bezier(0.2, 1.3, 0.4, 1);
		text-align: center;
	}
	.toast.warn {
		background: var(--warn);
		color: #1b1200;
	}
</style>
