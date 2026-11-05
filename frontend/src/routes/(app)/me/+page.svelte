<script lang="ts">
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { api, ApiError } from '#lib/api.ts';
	import Avatar from '#lib/components/Avatar.svelte';

	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	const MAX_BIO = 280;

	let { data }: PageProps = $props();
	// svelte-ignore state_referenced_locally
	let bio = $state(data.bio);
	let saving = $state(false);
	let uploading = $state(false);
	let file = $state<HTMLInputElement>();

	async function save(e: SubmitEvent) {
		e.preventDefault();
		saving = true;
		try {
			const res = await api<{ bio: string }>('/me/profile', { method: 'PUT', json: { bio } });
			bio = res.bio;
			toasts.show(i18n.t('me.saved'));
		} catch (err) {
			toasts.show(i18n.error(err), 'warn');
		} finally {
			saving = false;
		}
	}
	/** The image goes as the raw body: the server decodes, crops and resizes it */
	async function upload() {
		const f = file?.files?.[0];
		if (!f) return;
		uploading = true;
		try {
			const res = await fetch('/api/me/avatar', {
				method: 'PUT',
				body: f,
				headers: { 'content-type': f.type || 'application/octet-stream' }
			});
			const body = await res.json().catch(() => null);
			if (!res.ok) throw new ApiError(res.status, body?.error ?? 'internal');
			session.avatar = body.avatar;
			toasts.show(i18n.t('me.avatar_saved'));
		} catch (err) {
			toasts.show(i18n.error(err), 'warn');
		} finally {
			uploading = false;
			if (file) file.value = '';
		}
	}
	async function removeAvatar() {
		try {
			await api('/me/avatar', { method: 'DELETE' });
			session.avatar = null;
		} catch (err) {
			toasts.show(i18n.error(err), 'warn');
		}
	}
</script>

<svelte:head><title>{i18n.t('me.title')} · AnyCardz</title></svelte:head>

<div class="screen">
	<h1>{i18n.t('me.title')}</h1>

	<section class="panel" aria-labelledby="avatar-title">
		<h2 id="avatar-title">{i18n.t('me.avatar')}</h2>
		<div class="avatar-row">
			<Avatar name={session.username} src={session.avatar} size={96} />
			<div class="avatar-actions">
				<label class="btn" class:busy={uploading}>
					{i18n.t('me.upload')}
					<input
						id="avatar-file"
						bind:this={file}
						type="file"
						accept="image/*"
						onchange={upload}
						disabled={uploading}
					/>
				</label>
				{#if session.avatar}<button type="button" class="btn sm" onclick={removeAvatar}
						>{i18n.t('me.remove')}</button
					>{/if}
				<p class="hint">{i18n.t('me.avatar_hint')}</p>
			</div>
		</div>
	</section>

	<form class="panel" onsubmit={save}>
		<label for="bio"><h2>{i18n.t('me.bio')}</h2></label>
		<textarea id="bio" bind:value={bio} maxlength={MAX_BIO} rows="4"></textarea>
		<p class="hint">{i18n.t('me.bio_hint', { n: MAX_BIO - bio.length })}</p>
		<div class="row">
			<button class="btn primary" type="submit" disabled={saving}>{i18n.t('me.save')}</button>
			<a href="/players/{encodeURIComponent(session.username)}">{i18n.t('me.view')}</a>
		</div>
	</form>
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 24px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 640px;
		margin: 0 auto;
	}
	h1 {
		font-size: 28px;
	}
	h2 {
		font-size: 17px;
	}
	.panel {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 18px;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.avatar-row {
		display: flex;
		gap: 16px;
		align-items: center;
		flex-wrap: wrap;
	}
	.avatar-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		align-items: flex-start;
	}
	label.btn {
		position: relative;
		overflow: hidden;
	}
	label.btn input {
		position: absolute;
		inset: 0;
		opacity: 0;
		cursor: pointer;
	}
	label.btn:focus-within {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	.busy {
		opacity: 0.6;
	}
	textarea {
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 12px;
		padding: 10px 12px;
		font: inherit;
		font-size: 15px;
		resize: vertical;
	}
	.hint {
		color: var(--muted);
		font-size: 12.5px;
	}
	.row {
		display: flex;
		gap: 12px;
		align-items: center;
		flex-wrap: wrap;
	}
	.row a {
		color: var(--muted);
		font-size: 13px;
	}
</style>
