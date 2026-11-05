<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import type { LayoutProps } from './$types';
	import { setI18n } from '#lib/i18n/index.svelte.ts';
	import { setTheme } from '#lib/theme.svelte.ts';
	import { setToasts } from '#lib/session.svelte.ts';
	import Toasts from '#lib/components/Toasts.svelte';

	let { data, children }: LayoutProps = $props();
	// Initial values only: the switchers update these states themselves
	// svelte-ignore state_referenced_locally
	setI18n(data.lang);
	// svelte-ignore state_referenced_locally
	setTheme(data.theme);
	setToasts();
	// Mounted last, after every child: browser tests wait for this before clicking
	onMount(() => {
		document.documentElement.dataset.hydrated = '';
	});
</script>

<svelte:head>
	<title>AnyCardz</title>
</svelte:head>

{@render children()}
<Toasts />
