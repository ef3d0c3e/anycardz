<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession } from '#lib/session.svelte.ts';
	import { ago } from '#lib/live.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import type { NotificationDto } from '#lib/types.ts';
	import Avatar from './Avatar.svelte';
	import { featHref, featLine } from '#lib/progress.ts';
	import { getCatalog } from '#lib/session.svelte.ts';

	const i18n = getI18n();
	const session = getSession();
	const catalog = getCatalog();
	let open = $state(false);
	let items = $state<NotificationDto[]>([]);

	export function hrefOf(n: NotificationDto) {
		if (n.kind === 'achievement') return featHref(n.detail ?? '');
		if (n.kind === 'friend_request') return '/friends';
		if (n.kind === 'friend_accepted') return `/players/${encodeURIComponent(n.actor ?? '')}`;
		return n.trade ? `/trade/${n.trade}` : '/trade';
	}

	async function toggle() {
		open = !open;
		if (!open) return;
		const res = await api<{ items: NotificationDto[]; unread: number }>('/notifications').catch(
			() => null
		);
		if (!res) return;
		items = res.items;
		// Seeing the list reads it; the dots stay until the next opening
		if (res.unread) {
			session.notifications = 0;
			post('/notifications/read').catch(() => {});
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="bell-wrap">
	<button
		type="button"
		class="bell"
		aria-expanded={open}
		aria-label={session.notifications
			? i18n.t('notif.unread', { n: session.notifications })
			: i18n.t('notif.open')}
		onclick={toggle}
	>
		<svg viewBox="0 0 24 24" aria-hidden="true"
			><path d="M6 16V11a6 6 0 0 1 12 0v5l2 2H4zM10 20a2 2 0 0 0 4 0" /></svg
		>
		{#if session.notifications}<span class="badge">{session.notifications}</span>{/if}
	</button>
	{#if open}
		<button
			class="scrim"
			type="button"
			aria-label={i18n.t('common.close')}
			onclick={() => (open = false)}
		></button>
		<div class="list" role="dialog" aria-label={i18n.t('notif.title')}>
			<p class="eyebrow">{i18n.t('notif.title')}</p>
			<ul>
				{#each items as n (n.id)}
					<li>
						<a href={hrefOf(n)} class:unread={!n.read} onclick={() => (open = false)}>
							{#if n.kind === 'achievement'}
								<span class="medal" aria-hidden="true">★</span>
							{:else}
								<Avatar name={n.actor ?? '?'} src={n.actor_avatar} size={32} />
							{/if}
							<span class="text">
								{n.kind === 'achievement'
									? featLine(i18n, catalog, n.detail ?? '')
									: i18n.t(`notif.${n.kind}`, { name: n.actor ?? '?' })}
								<time datetime={n.at}>{ago(n.at, i18n.lang)}</time>
							</span>
						</a>
					</li>
				{:else}
					<li class="none">{i18n.t('notif.none')}</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.medal {
		width: 32px;
		height: 32px;
		flex: none;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: linear-gradient(135deg, #ffcf5c, #f08a24);
		color: #fff;
		font-size: 16px;
	}
	.bell-wrap {
		position: relative;
	}
	.bell {
		position: relative;
		display: grid;
		place-items: center;
		width: 38px;
		height: 38px;
		border-radius: 11px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
	}
	.bell svg {
		width: 20px;
		height: 20px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linejoin: round;
		stroke-linecap: round;
	}
	.badge {
		position: absolute;
		top: -5px;
		right: -5px;
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
	.scrim {
		position: fixed;
		inset: 0;
		background: transparent;
		border: 0;
		z-index: 30;
		cursor: default;
	}
	.list {
		position: absolute;
		right: -50px;
		top: calc(100% + 8px);
		z-index: 31;
		width: min(340px, calc(100vw - 24px));
		max-height: 70vh;
		overflow: auto;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 16px;
		box-shadow: var(--shadow);
		padding: 12px;
		animation: fade 0.15s;
	}
	ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	a {
		display: flex;
		gap: 10px;
		align-items: center;
		padding: 8px;
		border-radius: 10px;
		color: var(--ink);
		text-decoration: none;
		font-size: 13.5px;
	}
	a:hover {
		background: var(--surface-2);
	}
	a.unread {
		background: var(--accent-soft);
	}
	.text {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}
	time {
		color: var(--muted);
		font-size: 11.5px;
	}
	.none {
		color: var(--muted);
		font-size: 13px;
		padding: 12px 4px;
	}
</style>
