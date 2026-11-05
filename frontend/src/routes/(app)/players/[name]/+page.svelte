<script lang="ts">
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog, getSession, getToasts } from '#lib/session.svelte.ts';
	import { getLive } from '#lib/live.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import type { Relation, TradeDto } from '#lib/types.ts';
	import Avatar from '#lib/components/Avatar.svelte';
	import { featHref, featName, parseKey } from '#lib/progress.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const catalog = getCatalog()!;
	const session = getSession();
	const toasts = getToasts();
	const live = getLive();

	const p = $derived(data.profile);
	let relation = $state<Relation>('none');
	$effect(() => {
		relation = data.profile.relation;
	});
	let busy = $state(false);
	let confirming = $state(false);

	const joined = $derived(
		new Date(p.joined).toLocaleDateString(i18n.lang === 'fr' ? 'fr-FR' : 'en-US', {
			dateStyle: 'long'
		})
	);
	const name = $derived(p.username);

	async function act(method: 'POST' | 'DELETE', path: string, toast?: string) {
		busy = true;
		try {
			const res = await api<{ relation: Relation }>(path, {
				method,
				json: method === 'POST' ? {} : undefined
			});
			relation = res.relation;
			if (toast) toasts.show(toast);
			// Friend counts and block lists live elsewhere too
			if (path.startsWith('/blocks')) await live.loadBlocks();
			const me = await api<import('#lib/types.ts').MeDto>('/me').catch(() => null);
			if (me) session.update(me);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		} finally {
			busy = false;
			confirming = false;
		}
	}
	const friendPath = $derived(`/friends/${encodeURIComponent(name)}`);
	const blockPath = $derived(`/blocks/${encodeURIComponent(name)}`);
	async function trade() {
		try {
			const t = await post<TradeDto>('/trades', { with: name });
			goto(`/trade/${t.id}`);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
</script>

<svelte:head><title>{name} · AnyCardz</title></svelte:head>

<div class="screen">
	<header class="head">
		<Avatar {name} src={p.avatar} size={84} online={relation === 'self' ? undefined : p.online} />
		<div class="who">
			<h1>{name}</h1>
			<p class="meta">
				{#if relation !== 'self'}<span class:on={p.online}
						>{p.online ? i18n.t('profile.online') : i18n.t('profile.offline')}</span
					> ·{/if}
				{i18n.t('profile.joined', { date: joined })}
			</p>
		</div>
	</header>

	<p class="bio" class:empty={!p.bio}>{p.bio || i18n.t('profile.no_bio')}</p>

	<div class="actions">
		{#if relation === 'self'}
			<a class="btn" href="/me">{i18n.t('profile.edit')}</a>
		{:else if relation === 'blocked'}
			<p class="notice">{i18n.t('profile.blocked', { name })}</p>
			<button
				type="button"
				class="btn"
				disabled={busy}
				onclick={() => act('DELETE', blockPath, i18n.t('profile.unblocked_toast', { name }))}
				>{i18n.t('profile.unblock')}</button
			>
		{:else}
			{#if relation === 'none'}
				<button
					type="button"
					class="btn primary"
					disabled={busy}
					onclick={() => act('POST', friendPath)}>{i18n.t('profile.add')}</button
				>
			{:else if relation === 'request_sent'}
				<span class="state">{i18n.t('profile.sent')}</span>
				<button
					type="button"
					class="btn sm"
					disabled={busy}
					onclick={() => act('DELETE', friendPath)}>{i18n.t('profile.cancel')}</button
				>
			{:else if relation === 'request_received'}
				<button
					type="button"
					class="btn primary"
					disabled={busy}
					onclick={() => act('POST', friendPath, i18n.t('friends.accepted', { name }))}
					>{i18n.t('profile.accept')}</button
				>
				<button
					type="button"
					class="btn sm"
					disabled={busy}
					onclick={() => act('DELETE', friendPath)}>{i18n.t('profile.decline')}</button
				>
			{:else if relation === 'friends'}
				<span class="state friends">✓ {i18n.t('profile.friends')}</span>
				<button
					type="button"
					class="btn sm"
					disabled={busy}
					onclick={() => act('DELETE', friendPath)}>{i18n.t('profile.unfriend')}</button
				>
			{/if}
			<button type="button" class="btn" onclick={trade}>{i18n.t('profile.trade', { name })}</button>
			{#if confirming}
				<div class="confirm" role="alertdialog" aria-label={i18n.t('profile.block')}>
					<p>{i18n.t('profile.confirm_block', { name })}</p>
					<button
						type="button"
						class="btn sm danger"
						disabled={busy}
						onclick={() => act('POST', blockPath, i18n.t('profile.blocked_toast', { name }))}
						>{i18n.t('profile.confirm_yes')}</button
					>
					<button type="button" class="btn sm" onclick={() => (confirming = false)}
						>{i18n.t('profile.confirm_no')}</button
					>
				</div>
			{:else}
				<button type="button" class="linkish" onclick={() => (confirming = true)}
					>{i18n.t('profile.block')}</button
				>
			{/if}
		{/if}
	</div>

	<dl class="stats">
		<div>
			<dt>{i18n.t('profile.cards')}</dt>
			<dd class="mono">{i18n.num(p.cards)}</dd>
		</div>
		<div>
			<dt>{i18n.t('profile.copies')}</dt>
			<dd class="mono">{i18n.num(p.copies)}</dd>
		</div>
		<div>
			<dt>{i18n.t('profile.foils')}</dt>
			<dd class="mono">{i18n.num(p.foils)}</dd>
		</div>
	</dl>

	<section aria-labelledby="badges">
		<h2 id="badges">{i18n.t('profile.badges')}</h2>
		{#if p.achievements.length}
			<ul class="badges">
				{#each p.achievements as a (a.key)}
					{@const k = parseKey(a.key)}
					{@const col = k.kind === 'other' ? undefined : catalog.get(k.id)}
					<li>
						<a
							class="badge"
							class:foil={k.kind === 'foilset'}
							href={featHref(a.key)}
							style:--cp={col?.primary}
							style:--cs={col?.secondary}
							title={i18n.date(a.at)}
						>
							<span class="star" aria-hidden="true">★</span>
							{featName(i18n, catalog, a.key)}
						</a>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">{i18n.t('profile.no_badges')}</p>
		{/if}
	</section>

	<section aria-labelledby="cols">
		<h2 id="cols">{i18n.t('profile.collections')}</h2>
		{#if p.collections.length}
			<ul class="cols">
				{#each p.collections as c (c.slug)}
					{@const col = catalog.get(c.slug)}
					<li style:--cp={col?.primary} style:--cs={col?.secondary}>
						<span class="name">{col?.title ?? c.slug}</span>
						<span class="mono">{c.owned}/{c.total}</span>
						<span class="bar"><i style:width="{(c.owned / c.total) * 100}%"></i></span>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">{i18n.t('profile.no_cards')}</p>
		{/if}
	</section>
</div>

<style>
	.badges {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
	.badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px 6px 6px;
		border-radius: 999px;
		color: #fff;
		font-weight: 700;
		font-size: 13px;
		text-decoration: none;
		background: linear-gradient(120deg, var(--cp, #6d4cff), var(--cs, #ff4fa3));
		box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.25);
		text-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
	}
	.badge.foil {
		background:
			linear-gradient(115deg, transparent 30%, rgba(255, 255, 255, 0.45) 50%, transparent 70%),
			linear-gradient(120deg, var(--cp, #6d4cff), #ffcf5c 55%, var(--cs, #ff4fa3));
	}
	.star {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: rgba(255, 255, 255, 0.25);
		font-size: 12px;
	}
	.screen {
		display: flex;
		flex-direction: column;
		gap: 18px;
		padding: 24px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 760px;
		margin: 0 auto;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 16px;
	}
	.who {
		min-width: 0;
	}
	h1 {
		font-size: 30px;
		overflow-wrap: anywhere;
	}
	.meta {
		color: var(--muted);
		font-size: 13px;
		margin-top: 4px;
	}
	.meta .on {
		color: var(--ok);
		font-weight: 700;
	}
	.bio {
		font-size: 15px;
		max-width: 60ch;
		white-space: pre-line;
	}
	.bio.empty,
	.muted {
		color: var(--muted);
	}
	.actions {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
		align-items: center;
	}
	.state {
		font-weight: 700;
		color: var(--muted);
		font-size: 13.5px;
	}
	.state.friends {
		color: var(--ok);
	}
	.notice {
		color: var(--muted);
		font-size: 13.5px;
		flex-basis: 100%;
	}
	.confirm {
		display: flex;
		gap: 8px;
		align-items: center;
		flex-wrap: wrap;
		padding: 10px 12px;
		border-radius: 12px;
		background: var(--surface-2);
		border: 1px solid var(--line);
		font-size: 13.5px;
	}
	.danger {
		background: var(--danger);
		color: #fff;
		border-color: var(--danger);
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 10px;
		margin: 0;
	}
	.stats div {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 14px;
		padding: 12px;
		text-align: center;
	}
	dt {
		color: var(--muted);
		font-size: 12px;
		order: 2;
	}
	dd {
		margin: 0;
		font-size: 22px;
		font-weight: 700;
	}
	.stats div {
		display: flex;
		flex-direction: column-reverse;
	}
	h2 {
		font-size: 17px;
		margin-bottom: 10px;
	}
	.cols {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.cols li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 4px 10px;
		font-size: 13.5px;
	}
	.name {
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.cols .mono {
		color: var(--muted);
		font-size: 12px;
	}
	.bar {
		grid-column: 1 / -1;
		height: 6px;
		border-radius: 3px;
		background: var(--surface-2);
		overflow: hidden;
	}
	.bar i {
		display: block;
		height: 100%;
		background: linear-gradient(90deg, var(--cp), var(--cs));
	}
</style>
