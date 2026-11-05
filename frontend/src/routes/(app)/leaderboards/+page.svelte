<script lang="ts">
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { BOARDS, type Board, type Period, type RankDto } from '#lib/types.ts';
	import Avatar from '#lib/components/Avatar.svelte';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const href = (board: Board, period: Period) =>
		`/leaderboards?board=${board}${period === 'week' && board !== 'streak' ? '&period=week' : ''}`;
	const unit = $derived(i18n.t(`lb.unit.${data.board}` as never));
	const meOutside = $derived(
		data.ranking.me && !data.ranking.top.some((r) => r.me) ? data.ranking.me : null
	);
</script>

<svelte:head><title>{i18n.t('lb.title')} · AnyCardz</title></svelte:head>

{#snippet row(r: RankDto)}
	<li class="row" class:me={r.me} class:podium={r.rank <= 3}>
		<span
			class="rank mono"
			class:gold={r.rank === 1}
			class:silver={r.rank === 2}
			class:bronze={r.rank === 3}>{r.rank}</span
		>
		<a class="who" href="/players/{encodeURIComponent(r.username)}">
			<Avatar name={r.username} src={r.avatar} size={32} />
			<span class="name"
				>{r.username}{#if r.me}<span class="you">{i18n.t('lb.you')}</span>{/if}</span
			>
		</a>
		<span class="value mono">{i18n.num(r.value)} <small>{unit}</small></span>
	</li>
{/snippet}

<div class="screen">
	<div>
		<h1>{i18n.t('lb.title')}</h1>
		<p class="lead">{i18n.t('lb.lead')}</p>
	</div>

	<nav class="boards" aria-label={i18n.t('lb.title')}>
		{#each BOARDS as b}
			<a href={href(b, data.period)} aria-current={b === data.board ? 'page' : undefined}
				>{i18n.t(`lb.${b}`)}</a
			>
		{/each}
	</nav>

	{#if data.board === 'streak'}
		<p class="muted">{i18n.t('lb.streak_all_time')}</p>
	{:else}
		<div class="seg" role="group" aria-label={i18n.t('lb.title')}>
			{#each ['week', 'all'] as const as p}
				<a
					class="seg-btn"
					href={href(data.board, p)}
					aria-current={p === data.period ? 'page' : undefined}>{i18n.t(`lb.${p}`)}</a
				>
			{/each}
		</div>
	{/if}

	{#if data.ranking.top.length}
		<ol class="list">
			{#each data.ranking.top as r (r.username)}
				{@render row(r)}
			{/each}
		</ol>
		{#if meOutside}
			<ol class="list mine">{@render row(meOutside)}</ol>
		{:else if !data.ranking.me}
			<p class="muted">{i18n.t('lb.not_ranked')}</p>
		{/if}
	{:else}
		<p class="muted empty">{i18n.t('lb.empty')}</p>
	{/if}
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 24px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 720px;
		margin: 0 auto;
	}
	h1 {
		font-size: 28px;
	}
	.lead,
	.muted {
		color: var(--muted);
		margin-top: 6px;
	}
	/* Wrapped, not scrolled: every board stays in sight */
	.boards {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.boards a {
		flex: none;
		padding: 7px 14px;
		border-radius: 999px;
		border: 1px solid var(--line);
		text-decoration: none;
		color: var(--ink);
		font-weight: 600;
		font-size: 14px;
	}
	.boards a[aria-current='page'] {
		background: var(--ink);
		color: var(--bg);
		border-color: transparent;
	}
	.seg {
		align-self: flex-start;
	}
	.seg-btn {
		text-decoration: none;
	}
	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.mine {
		border-top: 1px dashed var(--line);
		padding-top: 10px;
	}
	.row {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 12px;
		border-radius: 14px;
		background: var(--surface);
		border: 1px solid var(--line);
	}
	.row.me {
		border-color: var(--accent);
		box-shadow: inset 3px 0 0 var(--accent);
	}
	.rank {
		width: 30px;
		height: 30px;
		flex: none;
		display: grid;
		place-items: center;
		border-radius: 50%;
		font-weight: 800;
		font-size: 13px;
		color: var(--muted);
	}
	.rank.gold {
		background: linear-gradient(135deg, #ffe08a, #e0a526);
		color: #4a3300;
	}
	.rank.silver {
		background: linear-gradient(135deg, #f1f3f7, #aab2c0);
		color: #2c3340;
	}
	.rank.bronze {
		background: linear-gradient(135deg, #f3c8a0, #b8743e);
		color: #3d220c;
	}
	.who {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: center;
		gap: 10px;
		text-decoration: none;
		color: var(--ink);
		font-weight: 600;
	}
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.you {
		margin-left: 8px;
		font-size: 11px;
		font-weight: 700;
		color: var(--accent);
		border: 1px solid currentColor;
		border-radius: 999px;
		padding: 1px 7px;
	}
	.value {
		font-weight: 700;
		white-space: nowrap;
	}
	.value small {
		color: var(--muted);
		font-weight: 500;
	}
	.empty {
		text-align: center;
		padding: 24px;
		border: 1.5px dashed var(--line);
		border-radius: 14px;
	}
</style>
