<script lang="ts">
	import { invalidate } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import { ago } from '#lib/live.svelte.ts';
	import { deviceName } from '#lib/device.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();

	// One form busy at a time; errors sit under the form they belong to
	let busy = $state('');
	let errors = $state<Record<string, string>>({});

	async function run(form: string, task: () => Promise<void>) {
		busy = form;
		errors = {};
		try {
			await task();
		} catch (e) {
			errors = { [form]: i18n.error(e) };
		} finally {
			busy = '';
		}
	}

	let newEmail = $state('');
	let emailPassword = $state('');
	const changeEmail = (e: SubmitEvent) => {
		e.preventDefault();
		run('email', async () => {
			const r = await post<{ email: string }>('/account/email', {
				email: newEmail,
				password: emailPassword
			});
			session.email = r.email;
			newEmail = emailPassword = '';
			toasts.show(i18n.t('settings.email.done'));
		});
	};

	let current = $state('');
	let next = $state('');
	const changePassword = (e: SubmitEvent) => {
		e.preventDefault();
		run('password', async () => {
			await post('/account/password', { current, new: next });
			current = next = '';
			toasts.show(i18n.t('settings.password.done'));
			await invalidate('app:sessions');
		});
	};

	const others = $derived(data.sessions.filter((s) => !s.current).length);
	const revoke = (id: number) =>
		run('sessions', async () => {
			await api(`/account/sessions/${id}`, { method: 'DELETE' });
			toasts.show(i18n.t('settings.sessions.revoked', { n: 1 }));
			await invalidate('app:sessions');
		});
	const revokeOthers = () =>
		run('sessions', async () => {
			const r = await post<{ signed_out: number }>('/account/sessions/revoke-others');
			toasts.show(i18n.t('settings.sessions.revoked', { n: r.signed_out }));
			await invalidate('app:sessions');
		});

	let confirmName = $state('');
	let deletePassword = $state('');
	const deleteAccount = (e: SubmitEvent) => {
		e.preventDefault();
		run('delete', async () => {
			await post('/account/delete', { confirm: confirmName, password: deletePassword });
			location.assign('/login');
		});
	};
</script>

<svelte:head><title>{i18n.t('settings.title')} · AnyCardz</title></svelte:head>

<div class="screen">
	<h1>{i18n.t('settings.title')}</h1>

	<section class="panel" aria-labelledby="email-title">
		<h2 id="email-title">{i18n.t('settings.email.title')}</h2>
		<p class="muted">{i18n.t('settings.email.current', { email: session.email })}</p>
		<form onsubmit={changeEmail}>
			<label
				>{i18n.t('settings.email.new')}
				<input
					id="new-email"
					type="email"
					autocomplete="email"
					required
					bind:value={newEmail}
				/></label
			>
			<label
				>{i18n.t('settings.password.confirm')}
				<input
					id="email-password"
					type="password"
					autocomplete="current-password"
					required
					bind:value={emailPassword}
				/></label
			>
			{#if errors.email}<p class="err" role="alert">{errors.email}</p>{/if}
			<button class="btn" type="submit" disabled={busy !== ''}
				>{i18n.t('settings.email.submit')}</button
			>
		</form>
	</section>

	<section class="panel" aria-labelledby="password-title">
		<h2 id="password-title">{i18n.t('settings.password.title')}</h2>
		<form onsubmit={changePassword}>
			<!-- Lets password managers tie the new password to the account -->
			<input type="text" autocomplete="username" value={session.username} hidden readonly />
			<label
				>{i18n.t('settings.password.current')}
				<input
					id="current-password"
					type="password"
					autocomplete="current-password"
					required
					bind:value={current}
				/></label
			>
			<label
				>{i18n.t('settings.password.new')}
				<input
					id="new-password"
					type="password"
					autocomplete="new-password"
					required
					minlength="8"
					bind:value={next}
				/>
				<span class="hint">{i18n.t('auth.password_hint')}</span></label
			>
			{#if errors.password}<p class="err" role="alert">{errors.password}</p>{/if}
			<button class="btn" type="submit" disabled={busy !== ''}
				>{i18n.t('settings.password.submit')}</button
			>
		</form>
	</section>

	<section class="panel" aria-labelledby="sessions-title">
		<h2 id="sessions-title">{i18n.t('settings.sessions.title')}</h2>
		<p class="muted">{i18n.t('settings.sessions.lead')}</p>
		<ul class="sessions">
			{#each data.sessions as s (s.id)}
				<li class="session">
					<div class="what">
						<b>{deviceName(s.user_agent) ?? i18n.t('settings.sessions.unknown')}</b>
						{#if s.current}<span class="tag">{i18n.t('settings.sessions.this')}</span>{/if}
						<small
							>{i18n.t('settings.sessions.since', { date: i18n.date(s.created_at) })} · {i18n.t(
								'settings.sessions.seen',
								{ ago: ago(s.last_seen_at, i18n.lang) }
							)}</small
						>
					</div>
					{#if !s.current}
						<button type="button" class="btn sm" disabled={busy !== ''} onclick={() => revoke(s.id)}
							>{i18n.t('settings.sessions.revoke')}</button
						>
					{/if}
				</li>
			{/each}
		</ul>
		{#if errors.sessions}<p class="err" role="alert">{errors.sessions}</p>{/if}
		{#if others > 1}
			<button type="button" class="btn" disabled={busy !== ''} onclick={revokeOthers}
				>{i18n.t('settings.sessions.revoke_others')}</button
			>
		{/if}
	</section>

	<section class="panel" aria-labelledby="data-title">
		<h2 id="data-title">{i18n.t('settings.data.title')}</h2>
		<p class="muted">{i18n.t('settings.data.lead')}</p>
		<a class="btn" href="/api/account/export" download data-sveltekit-reload
			>{i18n.t('settings.data.export')}</a
		>
	</section>

	<section class="panel" aria-labelledby="legal-title">
		<h2 id="legal-title">{i18n.t('settings.legal.title')}</h2>
		<p class="links">
			<a href="/terms">{i18n.t('legal.terms')}</a>
			<a href="/privacy">{i18n.t('legal.privacy')}</a>
		</p>
	</section>

	<section class="panel danger" aria-labelledby="delete-title">
		<h2 id="delete-title">{i18n.t('settings.delete.title')}</h2>
		<p class="muted">{i18n.t('settings.delete.lead')}</p>
		<form onsubmit={deleteAccount}>
			<label
				>{i18n.t('settings.delete.confirm', { name: session.username })}
				<input
					id="delete-confirm"
					autocomplete="off"
					autocapitalize="off"
					spellcheck="false"
					required
					bind:value={confirmName}
				/></label
			>
			<label
				>{i18n.t('settings.password.confirm')}
				<input
					id="delete-password"
					type="password"
					autocomplete="current-password"
					required
					bind:value={deletePassword}
				/></label
			>
			{#if errors.delete}<p class="err" role="alert">{errors.delete}</p>{/if}
			<button
				class="btn delete"
				type="submit"
				disabled={busy !== '' ||
					confirmName.trim().toLowerCase() !== session.username.toLowerCase()}
				>{i18n.t('settings.delete.submit')}</button
			>
		</form>
	</section>
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
	.panel.danger {
		border-color: color-mix(in srgb, var(--danger) 45%, var(--line));
	}
	.muted {
		color: var(--muted);
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 12px;
		align-items: flex-start;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-weight: 600;
		font-size: 13.5px;
		width: 100%;
	}
	input {
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 10px;
		padding: 11px 12px;
		font-size: 16px;
		width: 100%;
	}
	.hint {
		font-weight: 400;
		color: var(--muted);
		font-size: 12.5px;
	}
	.err {
		color: var(--danger);
		font-weight: 600;
		font-size: 13.5px;
	}
	.sessions {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.session {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: 12px;
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 8px;
	}
	.what small {
		flex-basis: 100%;
		color: var(--muted);
	}
	.tag {
		font-size: 11.5px;
		font-weight: 700;
		color: var(--accent);
		border: 1px solid currentColor;
		border-radius: 999px;
		padding: 1px 8px;
	}
	.links {
		display: flex;
		gap: 16px;
		flex-wrap: wrap;
	}
	.links a {
		color: var(--accent);
	}
	.btn.delete {
		background: var(--danger);
		border-color: transparent;
		color: #fff;
	}
	.btn.delete:disabled {
		opacity: 0.45;
	}
</style>
