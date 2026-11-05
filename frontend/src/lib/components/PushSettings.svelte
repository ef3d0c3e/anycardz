<script lang="ts">
	import { onMount } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { pushState, turnOff, turnOn, type PushState } from '#lib/push.ts';

	/** Push notifications for this device: one switch, and why it is grey when it is */
	const i18n = getI18n();
	let status = $state<PushState | null>(null);
	let key: string | null = null;
	let busy = $state(false);
	let error = $state('');
	// iOS delivers push to sites added to the home screen only
	let needsInstall = $state(false);

	onMount(async () => {
		needsInstall =
			/iPhone|iPad|iPod/.test(navigator.userAgent) &&
			!window.matchMedia('(display-mode: standalone)').matches;
		try {
			({ state: status, key } = await pushState());
		} catch (e) {
			error = i18n.error(e);
			status = 'off';
		}
	});

	async function toggle() {
		busy = true;
		error = '';
		try {
			status = status === 'on' ? await turnOff() : await turnOn(key!);
		} catch (e) {
			error = i18n.error(e);
		} finally {
			busy = false;
		}
	}
</script>

<section class="panel" aria-labelledby="push-title">
	<h2 id="push-title">{i18n.t('settings.push.title')}</h2>
	<p class="muted">{i18n.t('settings.push.lead')}</p>
	{#if status === 'unsupported'}
		<p class="note">
			{i18n.t(needsInstall ? 'settings.push.ios' : 'settings.push.unsupported')}
		</p>
	{:else if status === 'off-server'}
		<p class="note">{i18n.t('settings.push.off_server')}</p>
	{:else if status === 'blocked'}
		<p class="note">{i18n.t('settings.push.blocked')}</p>
	{:else if status}
		<div class="row">
			<span class="dot" class:on={status === 'on'} aria-hidden="true"></span>
			<span>{i18n.t(status === 'on' ? 'settings.push.state_on' : 'settings.push.state_off')}</span>
			<button type="button" class="btn sm" disabled={busy} onclick={toggle}
				>{i18n.t(status === 'on' ? 'settings.push.off' : 'settings.push.on')}</button
			>
		</div>
	{/if}
	{#if error}<p class="err" role="alert">{error}</p>{/if}
</section>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--line);
	}
	.dot.on {
		background: var(--ok);
	}
	.note {
		font-size: 13.5px;
		padding: 10px 12px;
		border-radius: 12px;
		background: var(--surface-2);
	}
	.err {
		color: var(--warn);
		font-size: 13.5px;
	}
</style>
