<script lang="ts">
	import { invalidate } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { api } from '#lib/api.ts';
	import type { FriendDto, MeDto } from '#lib/types.ts';
	import Avatar from '#lib/components/Avatar.svelte';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();

	let q = $state('');
	let found = $state<{ username: string }[]>([]);
	let timer: ReturnType<typeof setTimeout>;
	$effect(() => {
		const query = q.trim();
		clearTimeout(timer);
		if (!query) return void (found = []);
		timer = setTimeout(async () => {
			found = await api<{ username: string }[]>(`/players?q=${encodeURIComponent(query)}`).catch(
				() => []
			);
		}, 200);
	});

	async function act(method: 'POST' | 'DELETE', name: string, toast?: string) {
		try {
			await api(`/friends/${encodeURIComponent(name)}`, {
				method,
				json: method === 'POST' ? {} : undefined
			});
			if (toast) toasts.show(toast);
			await invalidate('app:friends');
			const me = await api<MeDto>('/me').catch(() => null);
			if (me) session.update(me);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
	const known = $derived(
		new Set(
			[...data.friends.friends, ...data.friends.incoming, ...data.friends.outgoing].map(
				(f) => f.username
			)
		)
	);
</script>

<svelte:head><title>{i18n.t('friends.title')} · AnyCardz</title></svelte:head>

{#snippet person(f: FriendDto)}
	<a class="person" href="/players/{encodeURIComponent(f.username)}">
		<Avatar name={f.username} src={f.avatar} size={34} online={f.online} />
		<span>{f.username}</span>
	</a>
{/snippet}

<div class="screen">
	<h1>{i18n.t('friends.title')}</h1>

	{#if data.friends.incoming.length}
		<section aria-labelledby="req">
			<h2 id="req">{i18n.t('friends.requests')}</h2>
			<ul>
				{#each data.friends.incoming as f (f.username)}
					<li class="row incoming">
						{@render person(f)}
						<button
							type="button"
							class="btn sm primary"
							onclick={() =>
								act('POST', f.username, i18n.t('friends.accepted', { name: f.username }))}
							>{i18n.t('profile.accept')}</button
						>
						<button type="button" class="btn sm" onclick={() => act('DELETE', f.username)}
							>{i18n.t('profile.decline')}</button
						>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	<section aria-labelledby="list">
		<h2 id="list">{i18n.t('friends.list')}</h2>
		{#if data.friends.friends.length}
			<ul>
				{#each data.friends.friends as f (f.username)}
					<li class="row">{@render person(f)}</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">{i18n.t('friends.none')}</p>
		{/if}
	</section>

	<section class="panel" aria-labelledby="find">
		<h2 id="find">{i18n.t('friends.search')}</h2>
		<input
			id="friend-search"
			type="search"
			bind:value={q}
			placeholder={i18n.t('trades.search')}
			aria-label={i18n.t('friends.search')}
			autocomplete="off"
		/>
		{#each found.filter((p) => !known.has(p.username)) as p (p.username)}
			<div class="row">
				{@render person({ username: p.username, avatar: null, online: false })}
				<button
					type="button"
					class="btn sm"
					onclick={() => act('POST', p.username, i18n.t('friends.requested', { name: p.username }))}
					>{i18n.t('friends.add', { name: p.username })}</button
				>
			</div>
		{/each}
	</section>

	{#if data.friends.outgoing.length}
		<section aria-labelledby="sent">
			<h2 id="sent">{i18n.t('friends.sent')}</h2>
			<ul>
				{#each data.friends.outgoing as f (f.username)}
					<li class="row">
						{@render person(f)}
						<button type="button" class="btn sm" onclick={() => act('DELETE', f.username)}
							>{i18n.t('profile.cancel')}</button
						>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 20px;
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
		margin-bottom: 10px;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border-radius: 14px;
		background: var(--surface);
		border: 1px solid var(--line);
	}
	.row.incoming {
		box-shadow: inset 3px 0 0 var(--accent);
	}
	.person {
		display: flex;
		align-items: center;
		gap: 10px;
		flex: 1;
		min-width: 0;
		text-decoration: none;
		color: var(--ink);
		font-weight: 600;
	}
	.person span {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.panel {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	input {
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 12px;
		padding: 11px 14px;
		font-size: 16px;
	}
	.muted {
		color: var(--muted);
	}
</style>
