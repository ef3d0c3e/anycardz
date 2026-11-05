<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { legalDoc, type LegalInfo } from '#lib/legal.ts';

	/** The terms or the privacy notice, readable logged in or not */
	interface Props {
		which: 'terms' | 'privacy';
		info: LegalInfo | null;
	}
	let { which, info }: Props = $props();
	const i18n = getI18n();
	const doc = $derived(legalDoc(i18n.lang, which, info));
	const other = $derived(which === 'terms' ? 'privacy' : 'terms');
</script>

<svelte:head><title>{doc.title} · AnyCardz</title></svelte:head>

<main class="legal">
	<nav class="top">
		<a class="logo" href="/">AnyCard<i>z</i></a>
		<a href="/{other}">{i18n.t(`legal.${other}`)}</a>
	</nav>
	<article>
		<h1>{doc.title}</h1>
		{#if info}
			<p class="meta">{i18n.t('legal.updated', { date: i18n.date(info.updated) })}</p>
		{/if}
		<p class="intro">{doc.intro}</p>
		{#each doc.sections as s, i}
			<section aria-labelledby="s{i}">
				<h2 id="s{i}">{s.title}</h2>
				{#each s.body as p}<p>{p}</p>{/each}
			</section>
		{/each}
	</article>
</main>

<style>
	.legal {
		min-height: 100dvh;
		background: var(--bg);
		color: var(--ink);
		padding: 20px max(16px, env(safe-area-inset-right, 0px)) 64px
			max(16px, env(safe-area-inset-left, 0px));
	}
	.top,
	article {
		max-width: 68ch;
		margin: 0 auto;
	}
	.top {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 28px;
		font-size: 14px;
	}
	.top a {
		color: var(--accent);
	}
	.logo {
		font: 800 22px/1 var(--display);
		letter-spacing: -0.03em;
		text-decoration: none;
		color: var(--ink) !important;
	}
	.logo i {
		font-style: normal;
		background: linear-gradient(110deg, #ff5fa8, #ffc24d 35%, #3ee6b5 60%, #5aa8ff 80%, #a77bff);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
	}
	h1 {
		font-size: clamp(26px, 6vw, 34px);
	}
	.meta {
		color: var(--muted);
		font-size: 13.5px;
		margin-top: 6px;
	}
	.intro {
		font-size: 17px;
		margin: 18px 0 8px;
	}
	h2 {
		font-size: 18px;
		margin: 28px 0 8px;
	}
	p {
		line-height: 1.6;
		margin: 0 0 10px;
	}
</style>
