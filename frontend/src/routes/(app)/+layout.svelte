<script lang="ts">
	import { page } from '$app/state';
	import { refreshAll } from '$app/navigation';
	import type { LayoutProps } from './$types';
	import { getI18n, LANGS, type Lang } from '#lib/i18n/index.svelte.ts';
	import { getTheme, setCookie, THEMES } from '#lib/theme.svelte.ts';
	import { setCatalog, setSession } from '#lib/session.svelte.ts';
	import { post } from '#lib/api.ts';
	import { setSound, soundOn } from '#lib/fx.ts';
	import Odometer from '#lib/components/Odometer.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import TopupSheet from '#lib/components/TopupSheet.svelte';
	import { setTopup } from '#lib/topup.svelte.ts';

	let { data, children }: LayoutProps = $props();
	const i18n = getI18n();
	const theme = getTheme();
	// svelte-ignore state_referenced_locally
	const session = setSession(data.me);
	// svelte-ignore state_referenced_locally
	const catalog = setCatalog(data.collections);
	const topup = setTopup();
	// Loads rerun after a language switch: keep the shared states in step
	$effect(() => {
		session.update(data.me);
		catalog.collections = data.collections;
	});

	let menuOpen = $state(false);
	let sound = $state(true);
	$effect(() => {
		sound = soundOn();
	});

	const tabs = [
		{ href: '/', key: 'nav.lobby', icon: 'lobby' },
		{ href: '/binder', key: 'nav.binder', icon: 'binder' },
		{ href: '/open', key: 'nav.open', icon: 'open' },
		{ href: '/forge', key: 'nav.forge', icon: 'forge' },
		{ href: '/trade', key: 'nav.trade', icon: 'trade' }
	] as const;
	const current = (href: string) =>
		href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);

	async function setLang(l: Lang) {
		setCookie('lang', l);
		i18n.lang = l;
		document.documentElement.lang = l;
		// Card and collection texts come from the API in the new language
		await refreshAll();
	}
	async function logout() {
		await post('/auth/logout').catch(() => {});
		location.assign('/login');
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (menuOpen = false)} />

<div class="shell">
	<header class="top">
		<a class="logo" href="/">AnyCard<i>z</i></a>
		<button
			type="button"
			class="wallet"
			id="wallet"
			aria-label={i18n.t('top.topup', { n: session.coins })}
			onclick={() => topup.ask()}
		>
			<span class="coin">c</span>
			<span class="sr">{i18n.t('top.coins', { n: session.coins })}</span>
			<Odometer value={session.coins} locale={i18n.lang === 'fr' ? 'fr-FR' : 'en-US'} />
			<span class="plus" aria-hidden="true">+</span>
		</button>
		<div class="account">
			<button
				type="button"
				class="avatar"
				aria-expanded={menuOpen}
				aria-label={i18n.t('menu.account', { name: session.username })}
				onclick={() => (menuOpen = !menuOpen)}>{session.username[0]?.toUpperCase()}</button
			>
			{#if menuOpen}
				<button
					class="scrim"
					type="button"
					aria-label={i18n.t('common.close')}
					onclick={() => (menuOpen = false)}
				></button>
				<div
					class="menu"
					role="dialog"
					aria-label={i18n.t('menu.account', { name: session.username })}
				>
					<p class="who"><b>{session.username}</b></p>
					<p class="eyebrow">{i18n.t('menu.theme')}</p>
					<div class="seg" role="group" aria-label={i18n.t('menu.theme')}>
						{#each THEMES as t}
							<button type="button" aria-pressed={theme.current === t} onclick={() => theme.set(t)}
								>{i18n.t(`theme.${t}`)}</button
							>
						{/each}
					</div>
					<p class="eyebrow">{i18n.t('menu.language')}</p>
					<div class="seg" role="group" aria-label={i18n.t('menu.language')}>
						{#each LANGS as l}
							<button
								type="button"
								aria-pressed={i18n.lang === l}
								lang={l}
								onclick={() => setLang(l)}>{l === 'en' ? 'English' : 'Français'}</button
							>
						{/each}
					</div>
					<p class="eyebrow">{i18n.t('menu.sound')}</p>
					<div class="seg" role="group" aria-label={i18n.t('menu.sound')}>
						<button type="button" aria-pressed={sound} onclick={() => setSound((sound = true))}
							>{i18n.t('menu.on')}</button
						>
						<button type="button" aria-pressed={!sound} onclick={() => setSound((sound = false))}
							>{i18n.t('menu.off')}</button
						>
					</div>
					<button type="button" class="btn sm logout" onclick={logout}
						>{i18n.t('menu.logout')}</button
					>
				</div>
			{/if}
		</div>
	</header>

	<nav class="nav" aria-label={i18n.t('nav.main')}>
		{#each tabs as tab}
			<a
				href={tab.href}
				class:open-tab={tab.icon === 'open'}
				aria-current={current(tab.href) ? 'page' : undefined}
				data-nav={tab.icon}
			>
				{#if tab.icon === 'open'}
					<span class="pack-ico"><Icon name="open" /></span>
					{#if session.pile.length}
						<span class="badge" aria-label={i18n.t('nav.packs_waiting', { n: session.pile.length })}
							>{session.pile.length}</span
						>
					{/if}
				{:else}
					<Icon name={tab.icon} />
				{/if}
				{i18n.t(tab.key)}
			</a>
		{/each}
	</nav>

	<main class="main">
		{@render children()}
	</main>
</div>

<TopupSheet />

<style>
	.shell {
		min-height: 100%;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: auto minmax(0, 1fr) auto;
		grid-template-areas: 'top' 'main' 'nav';
		height: 100dvh;
	}
	.top {
		grid-area: top;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: calc(10px + env(safe-area-inset-top, 0px)) max(14px, env(safe-area-inset-right, 0px))
			10px max(14px, env(safe-area-inset-left, 0px));
		background: var(--surface);
		border-bottom: 1px solid var(--line);
		z-index: 5;
	}
	.logo {
		font: 800 21px/1 var(--display);
		letter-spacing: -0.03em;
		text-decoration: none;
		color: var(--ink);
		margin-right: auto;
	}
	.logo i {
		font-style: normal;
		background: linear-gradient(110deg, #ff5fa8, #ffc24d 35%, #3ee6b5 60%, #5aa8ff 80%, #a77bff);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent;
	}
	.wallet {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 38px;
		padding: 0 12px 0 6px;
		border-radius: 999px;
		background: var(--surface-2);
		border: 1px solid var(--line);
		font: 700 15px/1 var(--mono);
	}
	.wallet {
		color: var(--ink);
		padding-right: 4px;
	}
	.wallet:hover {
		border-color: var(--legend);
	}
	.plus {
		width: 26px;
		height: 26px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: var(--legend);
		color: #2a1800;
		font: 800 17px/1 var(--display);
		margin-left: 4px;
	}
	.wallet:global(.ping) {
		animation: ping 0.6s;
	}
	@keyframes ping {
		30% {
			box-shadow: 0 0 0 4px color-mix(in srgb, var(--legend) 45%, transparent);
		}
	}
	.account {
		position: relative;
	}
	.avatar {
		width: 38px;
		height: 38px;
		border-radius: 50%;
		border: 0;
		display: grid;
		place-items: center;
		background: var(--accent);
		color: var(--accent-ink);
		font: 800 16px var(--display);
	}
	.scrim {
		position: fixed;
		inset: 0;
		background: transparent;
		border: 0;
		z-index: 30;
		cursor: default;
	}
	.menu {
		position: absolute;
		right: 0;
		top: calc(100% + 8px);
		z-index: 31;
		width: 270px;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 16px;
		box-shadow: var(--shadow);
		padding: 14px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		animation: fade 0.15s;
	}
	.menu .seg {
		align-self: flex-start;
		flex-wrap: wrap;
	}
	.who {
		font-size: 15px;
		padding-bottom: 6px;
		border-bottom: 1px solid var(--line);
	}
	.logout {
		margin-top: 6px;
		align-self: flex-start;
	}

	.nav {
		grid-area: nav;
		display: flex;
		justify-content: space-around;
		align-items: flex-end;
		background: var(--surface);
		border-top: 1px solid var(--line);
		padding: 6px max(6px, env(safe-area-inset-right, 0px))
			calc(8px + env(safe-area-inset-bottom, 0px)) max(6px, env(safe-area-inset-left, 0px));
		z-index: 5;
	}
	.nav a {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 3px;
		text-decoration: none;
		color: var(--muted);
		font-size: 11px;
		font-weight: 600;
		padding: 4px 8px;
		border-radius: 12px;
		min-width: 56px;
		position: relative;
	}
	.nav a[aria-current='page'] {
		color: var(--accent);
	}
	.pack-ico {
		width: 54px;
		height: 54px;
		margin-top: -26px;
		border-radius: 18px;
		display: grid;
		place-items: center;
		background: var(--ink);
		color: var(--bg);
		box-shadow: 0 6px 18px -6px rgba(0, 0, 0, 0.5);
		transform: rotate(-4deg);
		transition: transform 0.2s;
	}
	.pack-ico :global(svg) {
		width: 26px;
		height: 26px;
	}
	.open-tab:hover .pack-ico {
		transform: rotate(3deg) translateY(-2px);
	}
	.open-tab[aria-current='page'] .pack-ico {
		background: var(--accent);
		color: var(--accent-ink);
	}
	.badge {
		position: absolute;
		top: -30px;
		right: 2px;
		min-width: 18px;
		height: 18px;
		border-radius: 9px;
		padding: 0 5px;
		background: var(--danger);
		color: #fff;
		font: 700 11px/18px var(--mono);
		text-align: center;
		border: 2px solid var(--surface);
	}
	.main {
		grid-area: main;
		overflow: auto;
		overscroll-behavior: contain;
		min-width: 0;
		position: relative;
	}

	/* Tablet and up: the tab bar becomes a rail */
	@media (min-width: 700px) {
		.shell {
			grid-template-columns: 86px minmax(0, 1fr);
			grid-template-rows: auto minmax(0, 1fr);
			grid-template-areas: 'top top' 'nav main';
		}
		.nav {
			flex-direction: column;
			justify-content: flex-start;
			align-items: center;
			gap: 6px;
			border-top: 0;
			border-right: 1px solid var(--line);
			padding: 16px 6px;
		}
		.nav a {
			width: 72px;
			padding: 10px 4px;
		}
		.nav a.open-tab {
			order: -1;
			margin-bottom: 10px;
		}
		.pack-ico {
			margin-top: 0;
		}
		.badge {
			top: -6px;
			right: 0;
		}
		.nav a:not(.open-tab)[aria-current='page'] {
			background: var(--accent-soft);
		}
		.logo {
			margin-right: auto;
		}
	}
</style>
