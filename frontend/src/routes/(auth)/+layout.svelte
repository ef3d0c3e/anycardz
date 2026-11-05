<script lang="ts">
	import type { LayoutProps } from './$types';
	import Card from '#lib/components/Card.svelte';
	import { setCatalog } from '#lib/session.svelte.ts';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	const i18n = getI18n();
	let { data, children }: LayoutProps = $props();
	// svelte-ignore state_referenced_locally
	setCatalog(data.collections);
	const fan = $derived(data.collections.slice(0, 3).map((c) => c.featured));
</script>

<div class="auth">
	<div class="fan" aria-hidden="true">
		{#each fan as c, i}
			<div class="fan-card" style:--k={i}><Card card={c} /></div>
		{/each}
	</div>
	<div class="panel">
		<a class="logo" href="/login">AnyCard<i>z</i></a>
		{@render children()}
		<p class="legal">
			<a href="/terms">{i18n.t('legal.terms')}</a> ·
			<a href="/privacy">{i18n.t('legal.privacy')}</a>
		</p>
	</div>
</div>

<style>
	.auth {
		min-height: 100dvh;
		display: grid;
		place-items: center;
		gap: 24px;
		padding: 32px max(16px, env(safe-area-inset-right, 0px)) 32px
			max(16px, env(safe-area-inset-left, 0px));
		background: radial-gradient(ellipse at 50% 20%, var(--felt-hi), var(--felt) 70%);
	}
	@media (min-width: 900px) {
		.auth {
			grid-template-columns: 1fr 1fr;
			place-items: center;
		}
	}
	.fan {
		position: relative;
		width: min(220px, 50vw);
		aspect-ratio: 63/88;
	}
	.fan-card {
		position: absolute;
		inset: 0;
		transform: rotate(calc((var(--k) - 1) * 12deg)) translateX(calc((var(--k) - 1) * 30%));
		transform-origin: 50% 110%;
	}
	.panel {
		width: min(420px, 100%);
		background: var(--surface);
		border-radius: 24px;
		padding: 28px;
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
		display: flex;
		flex-direction: column;
		gap: 18px;
	}
	.logo {
		font: 800 24px/1 var(--display);
		letter-spacing: -0.03em;
		text-decoration: none;
		color: var(--ink);
	}
	.logo i {
		font-style: normal;
		background: linear-gradient(110deg, #ff5fa8, #ffc24d 35%, #3ee6b5 60%, #5aa8ff 80%, #a77bff);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
	}
	.panel :global(form) {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
	.panel :global(label) {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-weight: 600;
		font-size: 13.5px;
	}
	.panel :global(input) {
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 10px;
		padding: 11px 12px;
		font-size: 15px;
	}
	.panel :global(input[aria-invalid='true']) {
		border-color: var(--danger);
	}
	.panel :global(.hint) {
		font-weight: 400;
		color: var(--muted);
		font-size: 12.5px;
	}
	.panel :global(.err) {
		color: var(--danger);
		font-size: 13.5px;
		font-weight: 600;
	}
	.panel :global(.switch) {
		color: var(--muted);
		font-size: 13.5px;
	}
	.panel :global(h1) {
		font-size: 28px;
	}
	.legal {
		font-size: 12.5px;
		color: var(--muted);
		border-top: 1px solid var(--line);
		padding-top: 14px;
	}
	.legal a {
		color: var(--muted);
	}
	.panel :global(.lead) {
		color: var(--muted);
		margin-top: 6px;
	}
</style>
