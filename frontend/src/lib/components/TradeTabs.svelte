<script lang="ts">
	import { page } from '$app/state';
	import { getI18n } from '#lib/i18n/index.svelte.ts';

	/** Trades and the market: two ways to swap cards, one section */
	const i18n = getI18n();
	const tabs = [
		{ href: '/trade', key: 'market.tab_trades' },
		{ href: '/market', key: 'market.tab_market' }
	] as const;
</script>

<nav class="seg tabs" aria-label={i18n.t('market.tabs')}>
	{#each tabs as t (t.href)}
		<a href={t.href} aria-current={page.url.pathname.startsWith(t.href) ? 'page' : undefined}
			>{i18n.t(t.key)}</a
		>
	{/each}
</nav>

<style>
	.tabs {
		align-self: flex-start;
	}
	.tabs a {
		padding: 7px 16px;
		font-size: 13.5px;
	}
</style>
