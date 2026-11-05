<script lang="ts">
	import { page } from '$app/state';
	import { afterNavigate, invalidate, refreshAll } from '$app/navigation';
	import type { LayoutProps } from './$types';
	import { getI18n, LANGS, type Lang } from '#lib/i18n/index.svelte.ts';
	import { getTheme, setCookie, THEMES } from '#lib/theme.svelte.ts';
	import { setCatalog, setSession } from '#lib/session.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import { setSound, soundOn } from '#lib/fx.ts';
	import Odometer from '#lib/components/Odometer.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import TopupSheet from '#lib/components/TopupSheet.svelte';
	import { setTopup } from '#lib/topup.svelte.ts';
	import SidePanel from '#lib/components/SidePanel.svelte';
	import Avatar from '#lib/components/Avatar.svelte';
	import NotificationBell from '#lib/components/NotificationBell.svelte';
	import SearchPalette from '#lib/components/SearchPalette.svelte';
	import TermsGate from '#lib/components/TermsGate.svelte';
	import { getToasts } from '#lib/session.svelte.ts';
	import { setLive } from '#lib/live.svelte.ts';
	import { notifText } from '#lib/notify.ts';
	import { SOURCE_URL } from '#lib/source.ts';
	import type { MeDto } from '#lib/types.ts';
	import { onMount } from 'svelte';

	let { data, children }: LayoutProps = $props();
	const i18n = getI18n();
	const theme = getTheme();
	// svelte-ignore state_referenced_locally
	const session = setSession(data.me);
	// svelte-ignore state_referenced_locally
	const catalog = setCatalog(data.collections);
	const topup = setTopup();
	const live = setLive();
	const toasts = getToasts();
	// Several notifications can land together: only the latest answer counts
	let meCalls = 0;
	function refreshMe() {
		const call = ++meCalls;
		api<MeDto>('/me')
			.then((me) => {
				if (call !== meCalls) return;
				session.coins = me.coins;
				session.missions = me.missions;
				session.pile = me.pile;
			})
			.catch(() => {});
	}
	onMount(() => {
		live.start();
		// New notifications: the count, and a toast unless the page already shows it
		const off = live.onNotification((n) => {
			if (n.trade && page.url.pathname === `/trade/${n.trade}`) return;
			session.notifications++;
			if (n.kind === 'friend_request') session.friendRequests++;
			toasts.show(notifText(i18n, catalog, n));
			// Achievements, sales and missions pay (a mission bonus is a pack): refresh
			if (n.kind === 'achievement' || n.kind === 'market_sold' || n.kind === 'mission') {
				refreshMe();
			}
			if (n.kind === 'market_sold') invalidate('app:market');
		});
		return () => {
			off();
			live.stop();
		};
	});
	const latest = $derived(live.pulls[0]);
	// The sheet covers the page: going somewhere else closes it
	afterNavigate(() => {
		live.panelOpen = false;
		menuOpen = false;
	});
	// Loads rerun after a language switch: keep the shared states in step
	$effect(() => {
		session.update(data.me);
		catalog.collections = data.collections;
	});
	$effect(() => {
		catalog.freeTitle = i18n.t('free.pack');
	});

	let menuOpen = $state(false);
	let searching = $state(false);
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
		href === '/'
			? page.url.pathname === '/'
			: page.url.pathname.startsWith(href) ||
				// The market lives in the trade section
				(href === '/trade' && page.url.pathname.startsWith('/market'));

	async function setLang(l: Lang) {
		setCookie('lang', l);
		i18n.lang = l;
		document.documentElement.lang = l;
		// Card and collection texts come from the API in the new language
		await refreshAll();
		live.relocalize();
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
		<button
			type="button"
			class="search-btn"
			onclick={() => (searching = true)}
			aria-label={i18n.t('search.label')}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true"
				><circle cx="11" cy="11" r="6" /><path d="M20 20l-4.5-4.5" /></svg
			>
			<span class="search-text">{i18n.t('search.placeholder')}</span>
			<kbd>Ctrl K</kbd>
		</button>
		<NotificationBell />
		<button
			type="button"
			class="chat-btn"
			aria-label={i18n.t('live.open')}
			aria-expanded={live.panelOpen}
			onclick={() => (live.panelOpen = !live.panelOpen)}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v11H9l-5 4z" /></svg>
			{#if live.unread}<span class="badge" aria-label={i18n.t('live.unread', { n: live.unread })}
					>{live.unread}</span
				>{/if}
		</button>
		<div class="account">
			<button
				type="button"
				class="avatar"
				aria-expanded={menuOpen}
				aria-label={i18n.t('menu.account', { name: session.username })}
				onclick={() => (menuOpen = !menuOpen)}
				><Avatar name={session.username} src={session.avatar} size={38} /></button
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
					<nav class="menu-links">
						<a href="/players/{encodeURIComponent(session.username)}">{i18n.t('menu.profile')}</a>
						<a href="/friends"
							>{i18n.t('menu.friends')}{#if session.friendRequests}<span class="count"
									>{i18n.t('menu.requests', { n: session.friendRequests })}</span
								>{/if}</a
						>
						<a href="/achievements">{i18n.t('menu.achievements')}</a>
						<a href="/leaderboards">{i18n.t('menu.leaderboards')}</a>
						<a href="/settings">{i18n.t('menu.settings')}</a>
						<a href={SOURCE_URL} rel="noopener">{i18n.t('legal.source')}</a>
					</nav>
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

	{#if latest}
		{@const c = live.cards[latest.card]}
		<button type="button" class="ticker" onclick={() => (live.panelOpen = true)}>
			<span class="live">LIVE</span>
			{#key `${latest.at}${latest.card}`}
				<span class="ticker-text"
					><b>{latest.user}</b> ·
					<span class="r-tag-{latest.rarity}">{c?.title ?? latest.card}</span></span
				>
			{/key}
		</button>
	{/if}

	<main class="main">
		{@render children()}
	</main>

	<!-- A column on wide screens, a sheet over the page elsewhere -->
	<aside class="side" class:open={live.panelOpen} aria-label={i18n.t('live.panel')}>
		<SidePanel onClose={() => (live.panelOpen = false)} />
	</aside>
</div>

<TopupSheet />
<SearchPalette bind:open={searching} />
<!-- Settings stay reachable: export and deletion are the way out of new terms -->
{#if session.termsOutdated && page.url.pathname !== '/settings'}
	<TermsGate onLogout={logout} />
{/if}

<style>
	.shell {
		min-height: 100%;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		grid-template-rows: auto auto minmax(0, 1fr) auto;
		grid-template-areas: 'top' 'ticker' 'main' 'nav';
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
	/* Narrow phones: the controls stay, the logo gives way */
	@media (max-width: 479px) {
		.top {
			gap: 6px;
		}
		.logo {
			font-size: 18px;
			min-width: 0;
			overflow: hidden;
			white-space: nowrap;
			flex-shrink: 1;
		}
		.plus {
			display: none;
		}
		.wallet {
			padding-right: 10px;
		}
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
		position: relative;
		padding: 0;
		border: 0;
		background: none;
		border-radius: 50%;
		display: grid;
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
	.menu-links {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding-bottom: 6px;
		border-bottom: 1px solid var(--line);
	}
	.menu-links a {
		color: var(--ink);
		text-decoration: none;
		font-weight: 600;
		padding: 6px 4px;
		border-radius: 8px;
		display: flex;
		justify-content: space-between;
		gap: 8px;
	}
	.menu-links a:hover {
		background: var(--surface-2);
	}
	.menu-links .count {
		color: var(--accent);
		font-size: 12px;
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
	/* Phones: a dark tile inside the bar, the same height as the other icons'
	   row, so the tab never sticks out over the page */
	.pack-ico {
		width: 46px;
		height: 30px;
		border-radius: 10px;
		display: grid;
		place-items: center;
		background: var(--ink);
		color: var(--bg);
		box-shadow: 0 6px 18px -6px rgba(0, 0, 0, 0.5);
		transform: rotate(-4deg);
		transition: transform 0.2s;
	}
	.pack-ico :global(svg) {
		width: 20px;
		height: 20px;
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
		top: -4px;
		right: 0;
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
	.search-btn {
		display: grid;
		place-items: center;
		width: 38px;
		height: 38px;
		border-radius: 11px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--muted);
		flex: none;
	}
	.search-btn svg {
		width: 20px;
		height: 20px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
	}
	.search-text,
	.search-btn kbd {
		display: none;
	}
	/* Desktop: a search field look, with its shortcut */
	@media (min-width: 900px) {
		.search-btn {
			display: flex;
			gap: 10px;
			width: auto;
			min-width: 260px;
			padding: 0 10px;
			margin-right: auto;
			font-size: 13px;
			background: var(--surface-2);
		}
		.search-text {
			display: inline;
		}
		.search-btn kbd {
			display: inline;
			margin-left: auto;
			font: 600 11px var(--mono);
			border: 1px solid var(--line);
			border-radius: 5px;
			padding: 1px 5px;
			background: var(--surface);
		}
	}
	.chat-btn {
		position: relative;
		display: grid;
		place-items: center;
		width: 38px;
		height: 38px;
		border-radius: 11px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		flex: none;
	}
	.chat-btn svg {
		width: 20px;
		height: 20px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linejoin: round;
	}
	.chat-btn .badge {
		top: -5px;
		right: -5px;
	}
	.ticker {
		grid-area: ticker;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 7px 14px;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: var(--surface);
		font-size: 12.5px;
		white-space: nowrap;
		min-width: 0;
		text-align: left;
		color: var(--ink);
	}
	.ticker .live {
		font: 700 10px/1 var(--mono);
		letter-spacing: 0.1em;
		color: #fff;
		background: var(--danger);
		padding: 4px 6px;
		border-radius: 5px;
		flex: none;
	}
	.ticker-text {
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
		animation: tin 0.45s ease-out;
	}
	.ticker-text span {
		color: var(--rc);
		font-weight: 700;
	}
	@keyframes tin {
		from {
			transform: translateY(100%);
			opacity: 0;
		}
	}
	.side {
		position: fixed;
		inset: 0;
		z-index: 40;
		transform: translateY(102%);
		transition: transform 0.35s cubic-bezier(0.2, 0.9, 0.3, 1);
		visibility: hidden;
	}
	.side.open {
		transform: none;
		visibility: visible;
	}
	@media (min-width: 700px) {
		.shell {
			grid-template-columns: 86px minmax(0, 1fr);
			grid-template-rows: auto minmax(0, 1fr);
			grid-template-areas: 'top top' 'nav main';
		}
		.ticker {
			display: none;
		}
		.side {
			left: auto;
			width: 360px;
			border-left: 1px solid var(--line);
			box-shadow: var(--shadow);
			transform: translateX(102%);
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
		/* The rail has room: the big tile */
		.pack-ico {
			width: 54px;
			height: 54px;
			border-radius: 18px;
		}
		.pack-ico :global(svg) {
			width: 26px;
			height: 26px;
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
	/* Desktop: the panel is a column, always there */
	@media (min-width: 1080px) {
		.shell {
			grid-template-columns: 86px minmax(0, 1fr) 320px;
			grid-template-areas: 'top top top' 'nav main side';
		}
		.side {
			position: static;
			grid-area: side;
			transform: none;
			visibility: visible;
			width: auto;
			box-shadow: none;
			z-index: 1;
			min-height: 0;
		}
		.side :global(.x),
		.chat-btn {
			display: none;
		}
	}
</style>
