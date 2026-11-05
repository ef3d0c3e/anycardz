<script lang="ts">
	import { goto } from '$app/navigation';
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getToasts } from '#lib/session.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import type { TradeDto } from '#lib/types.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const toasts = getToasts();

	let q = $state('');
	let found = $state<{ username: string }[]>([]);
	let searched = $state('');
	let timer: ReturnType<typeof setTimeout>;
	$effect(() => {
		const query = q.trim();
		clearTimeout(timer);
		if (!query) {
			found = [];
			searched = '';
			return;
		}
		timer = setTimeout(async () => {
			found = await api<{ username: string }[]>(`/players?q=${encodeURIComponent(query)}`).catch(
				() => []
			);
			searched = query;
		}, 200);
	});

	async function start(name: string) {
		try {
			const t = await post<TradeDto>('/trades', { with: name });
			goto(`/trade/${t.id}`);
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		}
	}
	const open = $derived(data.trades.filter((t) => t.state === 'open'));
	const closed = $derived(data.trades.filter((t) => t.state !== 'open'));
	const cards = (n: number) => i18n.t('trade.cards', { n });
</script>

<svelte:head><title>{i18n.t('trades.title')} · AnyCardz</title></svelte:head>

<div class="screen">
	<div>
		<h1>{i18n.t('trades.title')}</h1>
		<p class="lead">{i18n.t('trades.lead')}</p>
	</div>

	<section class="panel" aria-labelledby="new-trade">
		<h2 id="new-trade">{i18n.t('trades.new')}</h2>
		<input
			id="player-search"
			type="search"
			bind:value={q}
			placeholder={i18n.t('trades.search')}
			aria-label={i18n.t('trades.search')}
			autocomplete="off"
		/>
		{#if found.length}
			<ul class="players">
				{#each found as p (p.username)}
					<li>
						<button type="button" class="player" onclick={() => start(p.username)}>
							<span class="avatar">{p.username[0].toUpperCase()}</span>
							<span>{i18n.t('trades.start', { name: p.username })}</span>
						</button>
					</li>
				{/each}
			</ul>
		{:else if searched}
			<p class="muted">{i18n.t('trades.no_match', { q: searched })}</p>
		{/if}
	</section>

	{#if open.length}
		<section aria-labelledby="open-trades">
			<h2 id="open-trades">{i18n.t('trades.open_list')}</h2>
			<ul class="trades">
				{#each open as t (t.id)}
					<li>
						<a class="trade" href="/trade/{t.id}" class:turn={!t.you_sealed}>
							<span class="avatar">{t.partner[0]?.toUpperCase()}</span>
							<span class="who">
								<b>{t.partner}</b>
								<small
									>{i18n.t('trades.summary', {
										mine: cards(t.mine),
										theirs: cards(t.theirs)
									})}</small
								>
							</span>
							<span class="state" class:yours={!t.you_sealed}>
								{t.you_sealed
									? i18n.t('trades.waiting_them', { name: t.partner })
									: i18n.t('trades.waiting_you')}
							</span>
						</a>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if closed.length}
		<section aria-labelledby="closed-trades">
			<h2 id="closed-trades">{i18n.t('trades.closed_list')}</h2>
			<ul class="trades">
				{#each closed as t (t.id)}
					<li>
						<a class="trade quiet" href="/trade/{t.id}">
							<span class="avatar">{t.partner[0]?.toUpperCase()}</span>
							<span class="who">
								<b>{t.partner}</b>
								<small
									>{i18n.t('trades.summary', {
										mine: cards(t.mine),
										theirs: cards(t.theirs)
									})}</small
								>
							</span>
							<span class="state"
								>{i18n.t(t.state === 'done' ? 'trades.done' : 'trades.cancelled')}</span
							>
						</a>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if !data.trades.length}
		<p class="muted empty">{i18n.t('trades.none')}</p>
	{/if}
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 20px;
		padding: 16px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 760px;
		margin: 0 auto;
	}
	@media (min-width: 700px) {
		.screen {
			padding: 24px;
		}
	}
	h1 {
		font-size: 28px;
	}
	h2 {
		font-size: 17px;
		margin-bottom: 10px;
	}
	.lead,
	.muted {
		color: var(--muted);
		max-width: 60ch;
		margin-top: 6px;
	}
	.panel {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 18px;
		padding: 16px;
	}
	input {
		width: 100%;
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 12px;
		padding: 12px 14px;
		font-size: 16px;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.players {
		margin-top: 10px;
	}
	.player,
	.trade {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		border-radius: 14px;
		border: 1px solid var(--line);
		background: var(--surface);
		text-align: left;
		text-decoration: none;
		color: var(--ink);
		font-weight: 600;
	}
	.player:hover,
	.trade:hover {
		background: var(--surface-2);
	}
	.trade.turn {
		box-shadow: inset 3px 0 0 var(--accent);
	}
	.trade.quiet {
		opacity: 0.75;
	}
	.avatar {
		width: 34px;
		height: 34px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: var(--rare);
		color: #fff;
		font: 800 14px var(--display);
		flex: none;
	}
	.who {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1;
	}
	.who small {
		color: var(--muted);
		font-weight: 500;
	}
	.state {
		font-size: 12px;
		color: var(--muted);
		text-align: right;
	}
	.state.yours {
		color: var(--accent);
		font-weight: 700;
	}
	.empty {
		text-align: center;
		padding: 24px;
		border: 1.5px dashed var(--line);
		border-radius: 14px;
	}
</style>
