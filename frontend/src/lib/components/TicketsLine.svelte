<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession } from '#lib/session.svelte.ts';

	/** Scratch tickets in hand, and when the next one comes */
	let { href = '/open' }: { href?: string | null } = $props();
	const i18n = getI18n();
	const session = getSession();
	const t = $derived(session.tickets);

	let left = $state('');
	function tick() {
		const next = t?.next_at;
		if (!next) return (left = '');
		const s = Math.max(0, Math.floor((Date.parse(next) - Date.now()) / 1000));
		left = `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}
	tick();
	const timer = setInterval(tick, 1000);
	onDestroy(() => clearInterval(timer));
</script>

{#if t}
	<p class="tickets-line">
		<span class="stub" aria-hidden="true">🎟</span>
		{#if t.count && href}
			<a {href}>{i18n.t('tickets.held', { n: t.count, max: t.max })}</a>
		{:else if t.count}
			<span>{i18n.t('tickets.held', { n: t.count, max: t.max })}</span>
		{/if}
		{#if left}
			<span class="next" class:alone={!t.count}
				>{i18n.t(t.count ? 'tickets.next' : 'tickets.next_alone', { time: left })}</span
			>
		{:else if t.count >= t.max}
			<span class="next">{i18n.t('tickets.full')}</span>
		{/if}
	</p>
{/if}

<style>
	.tickets-line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 8px;
		font-size: 13.5px;
	}
	.stub {
		font-size: 16px;
	}
	a,
	span:not(.stub, .next) {
		font-weight: 700;
		color: #ffcf5c;
		text-decoration: none;
	}
	.next {
		opacity: 0.75;
		font-variant-numeric: tabular-nums;
	}
	.next.alone {
		opacity: 0.9;
	}
</style>
