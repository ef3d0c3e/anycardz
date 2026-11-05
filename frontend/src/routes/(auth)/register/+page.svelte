<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { post } from '#lib/api.ts';
	import { TERMS_VERSION } from '#lib/legal.ts';
	import TermsSentence from '#lib/components/TermsSentence.svelte';

	const i18n = getI18n();
	let username = $state('');
	let email = $state('');
	let password = $state('');
	let accepted = $state(false);
	let code = $state('');
	let busy = $state(false);
	// Which field an error code points at, so the message sits under it
	const field = $derived(
		code.includes('username')
			? 'username'
			: code.includes('email')
				? 'email'
				: code.includes('password')
					? 'password'
					: code.startsWith('terms')
						? 'terms'
						: ''
	);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		code = '';
		try {
			await post('/auth/register', {
				username,
				email,
				password,
				terms: accepted ? TERMS_VERSION : null
			});
			location.assign('/');
		} catch (err) {
			code = (err as { code?: string }).code ?? 'internal';
			busy = false;
		}
	}
</script>

<svelte:head><title>{i18n.t('auth.register.submit')} · AnyCardz</title></svelte:head>

<div>
	<h1>{i18n.t('auth.register.title')}</h1>
	<p class="lead">{i18n.t('auth.register.lead')}</p>
</div>
<form onsubmit={submit} novalidate>
	<label>
		{i18n.t('auth.username')}
		<input
			id="username"
			name="username"
			autocomplete="username"
			required
			bind:value={username}
			aria-invalid={field === 'username'}
			aria-describedby="username-hint"
		/>
		{#if field === 'username'}<span class="err" role="alert">{i18n.error({ code })}</span>
		{:else}<span class="hint" id="username-hint">{i18n.t('auth.username_hint')}</span>{/if}
	</label>
	<label>
		{i18n.t('auth.email')}
		<input
			id="email"
			name="email"
			type="email"
			autocomplete="email"
			required
			bind:value={email}
			aria-invalid={field === 'email'}
		/>
		{#if field === 'email'}<span class="err" role="alert">{i18n.error({ code })}</span>{/if}
	</label>
	<label>
		{i18n.t('auth.password')}
		<input
			id="password"
			name="password"
			type="password"
			autocomplete="new-password"
			required
			bind:value={password}
			aria-invalid={field === 'password'}
			aria-describedby="password-hint"
		/>
		{#if field === 'password'}<span class="err" role="alert">{i18n.error({ code })}</span>
		{:else}<span class="hint" id="password-hint">{i18n.t('auth.password_hint')}</span>{/if}
	</label>
	<div class="terms">
		<label class="check">
			<input id="terms" type="checkbox" bind:checked={accepted} aria-invalid={field === 'terms'} />
			<span><TermsSentence key="legal.accept_label" /></span>
		</label>
		{#if field === 'terms'}<span class="err" role="alert">{i18n.error({ code })}</span>{/if}
	</div>
	{#if code && !field}<p class="err" role="alert">{i18n.error({ code })}</p>{/if}
	<button class="btn primary" type="submit" disabled={busy}
		>{busy ? i18n.t('auth.working') : i18n.t('auth.register.submit')}</button
	>
</form>
<p class="switch">
	{i18n.t('auth.register.has_account')} <a href="/login">{i18n.t('auth.register.login_link')}</a>
</p>

<style>
	.terms {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.check {
		flex-direction: row !important;
		align-items: flex-start;
		gap: 10px !important;
		font-weight: 500 !important;
		line-height: 1.45;
	}
	.check input {
		width: 20px;
		height: 20px;
		flex: none;
		margin: 0;
		accent-color: var(--accent);
	}
	.check :global(a) {
		color: var(--accent);
	}
</style>
