<script lang="ts">
	import { onMount } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { post } from '#lib/api.ts';
	import { TERMS_VERSION } from '#lib/legal.ts';
	import TermsSentence from './TermsSentence.svelte';

	/** Blocks the game until the current terms are accepted */
	let { onLogout }: { onLogout: () => void } = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	let busy = $state(false);
	let button = $state<HTMLButtonElement>();
	onMount(() => button?.focus());

	async function accept() {
		busy = true;
		try {
			await post('/legal/accept', { version: TERMS_VERSION });
			session.termsOutdated = false;
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
			busy = false;
		}
	}
</script>

<div class="gate">
	<div class="box" role="alertdialog" aria-modal="true" aria-labelledby="terms-gate-title">
		<h2 id="terms-gate-title">{i18n.t('legal.changed.title')}</h2>
		<p><TermsSentence key="legal.changed.lead" /></p>
		<button type="button" class="btn primary" bind:this={button} disabled={busy} onclick={accept}
			>{i18n.t('legal.changed.accept')}</button
		>
		<p class="small">
			{i18n.t('legal.changed.alternative')}
			<a href="/settings">{i18n.t('menu.settings')}</a> ·
			<button type="button" class="link" onclick={onLogout}>{i18n.t('menu.logout')}</button>
		</p>
	</div>
</div>

<style>
	.gate {
		position: fixed;
		inset: 0;
		z-index: 90;
		display: grid;
		place-items: center;
		padding: 16px;
		background: rgba(8, 10, 24, 0.72);
		backdrop-filter: blur(6px);
	}
	.box {
		width: min(440px, 100%);
		background: var(--surface);
		color: var(--ink);
		border-radius: 20px;
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 14px;
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
	}
	h2 {
		font-size: 22px;
	}
	p {
		line-height: 1.5;
	}
	.box :global(a),
	.link {
		color: var(--accent);
	}
	.small {
		color: var(--muted);
		font-size: 13px;
	}
	.link {
		background: none;
		border: 0;
		padding: 0;
		font: inherit;
		text-decoration: underline;
		cursor: pointer;
	}
</style>
