<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { post } from '#lib/api.ts';

	const i18n = getI18n();
	let login = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			await post('/auth/login', { login, password });
			// Full load: every layout picks up the new session
			location.assign('/');
		} catch (err) {
			error = i18n.error(err);
			busy = false;
		}
	}
</script>

<svelte:head><title>{i18n.t('auth.login.submit')} · AnyCardz</title></svelte:head>

<div>
	<h1>{i18n.t('auth.login.title')}</h1>
	<p class="lead">{i18n.t('auth.login.lead')}</p>
</div>
<form onsubmit={submit} novalidate>
	<label>
		{i18n.t('auth.login.field')}
		<input
			id="login"
			name="login"
			autocomplete="username"
			required
			bind:value={login}
			aria-invalid={!!error}
		/>
	</label>
	<label>
		{i18n.t('auth.password')}
		<input
			id="password"
			name="password"
			type="password"
			autocomplete="current-password"
			required
			bind:value={password}
			aria-invalid={!!error}
		/>
	</label>
	{#if error}<p class="err" role="alert">{error}</p>{/if}
	<button class="btn primary" type="submit" disabled={busy}
		>{busy ? i18n.t('auth.working') : i18n.t('auth.login.submit')}</button
	>
</form>
<p class="switch">
	{i18n.t('auth.login.no_account')} <a href="/register">{i18n.t('auth.login.register_link')}</a>
</p>
