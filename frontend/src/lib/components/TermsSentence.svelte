<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';

	/** A sentence with {terms} and {privacy} turned into links, whatever their
	 * order in the language */
	let { key }: { key: 'legal.accept_label' | 'legal.changed.lead' } = $props();
	const i18n = getI18n();
	const parts = $derived(
		i18n.t(key, { terms: '\u0001terms\u0001', privacy: '\u0001privacy\u0001' }).split('\u0001')
	);
</script>

{#each parts as part}{#if part === 'terms'}<a href="/terms" target="_blank"
			>{i18n.t('legal.terms_link')}</a
		>{:else if part === 'privacy'}<a href="/privacy" target="_blank"
			>{i18n.t('legal.privacy_link')}</a
		>{:else}{part}{/if}{/each}
