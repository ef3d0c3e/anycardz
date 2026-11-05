<script lang="ts">
	import type { PageProps } from './$types';
	import { getI18n } from '#lib/i18n/index.svelte.ts';

	let { data }: PageProps = $props();
	const i18n = getI18n();
	const items = $derived(data.achievements.items);
	const earned = $derived(items.filter((a) => a.earned_at).length);
	// Earned last, so what is left to do comes first; closest to done first
	const sorted = $derived(
		[...items].sort((a, b) => {
			if (!!a.earned_at !== !!b.earned_at) return a.earned_at ? 1 : -1;
			const pa = a.goal ? (a.have ?? 0) / a.goal : 0;
			const pb = b.goal ? (b.have ?? 0) / b.goal : 0;
			return pb - pa;
		})
	);
	const name = (id: string) => i18n.t(`ach.${id}` as never);
	const desc = (id: string) => i18n.t(`ach.${id}.desc` as never);
</script>

<svelte:head><title>{i18n.t('ach.title')} · AnyCardz</title></svelte:head>

<div class="screen">
	<div>
		<h1>{i18n.t('ach.title')}</h1>
		<p class="lead">{i18n.t('ach.lead')}</p>
		<p class="summary">
			{i18n.t('ach.summary', {
				earned,
				total: items.length,
				sets: data.achievements.sets,
				foil_sets: data.achievements.foil_sets
			})}
		</p>
		<a class="btn sm" href="/leaderboards">{i18n.t('menu.leaderboards')}</a>
	</div>

	<ul class="grid">
		{#each sorted as a (a.id)}
			{@const done = !!a.earned_at}
			<li class="ach" class:done id="ach-{a.id}">
				<span class="medal" aria-hidden="true">{done ? '★' : '☆'}</span>
				<div class="body">
					<h2>{name(a.id)}</h2>
					<p class="desc">{desc(a.id)}</p>
					{#if done}
						<p class="when">{i18n.t('ach.earned', { date: i18n.date(a.earned_at!) })}</p>
					{:else if a.goal}
						{@const have = Math.min(a.have ?? 0, a.goal)}
						<div
							class="bar"
							role="progressbar"
							aria-valuemin={0}
							aria-valuemax={a.goal}
							aria-valuenow={have}
						>
							<i style:width="{(have / a.goal) * 100}%"></i>
						</div>
						<p class="when mono">{i18n.t('ach.progress', { have, goal: a.goal })}</p>
					{/if}
				</div>
				<span class="coins mono">{i18n.t('ach.coins', { n: a.coins })}</span>
			</li>
		{/each}
	</ul>
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: 18px;
		padding: 24px max(16px, env(safe-area-inset-right, 0px)) 52px
			max(16px, env(safe-area-inset-left, 0px));
		max-width: 900px;
		margin: 0 auto;
	}
	h1 {
		font-size: 28px;
	}
	.lead,
	.summary {
		color: var(--muted);
		margin: 6px 0 10px;
	}
	.summary {
		font-weight: 600;
		color: var(--ink);
	}
	.grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 280px), 1fr));
		gap: 10px;
	}
	/* A notification's link lands here: the achievement stands out a moment */
	.ach:target {
		scroll-margin-top: 16px;
		box-shadow: 0 0 0 2px var(--accent);
		animation: flash 1.6s ease-out;
	}
	@keyframes flash {
		from {
			box-shadow: 0 0 0 6px color-mix(in srgb, var(--accent) 45%, transparent);
		}
	}
	.ach {
		display: flex;
		gap: 12px;
		align-items: flex-start;
		padding: 14px;
		border-radius: 16px;
		background: var(--surface);
		border: 1px solid var(--line);
	}
	.ach.done {
		border-color: color-mix(in srgb, #ffcf5c 55%, var(--line));
		background: color-mix(in srgb, #ffcf5c 8%, var(--surface));
	}
	.medal {
		width: 40px;
		height: 40px;
		flex: none;
		border-radius: 50%;
		display: grid;
		place-items: center;
		font-size: 20px;
		background: var(--surface-2);
		color: var(--muted);
	}
	.done .medal {
		background: linear-gradient(135deg, #ffcf5c, #f08a24);
		color: #fff;
	}
	.body {
		flex: 1;
		min-width: 0;
	}
	h2 {
		font-size: 15.5px;
	}
	.desc {
		color: var(--muted);
		font-size: 13.5px;
		margin-top: 2px;
	}
	.when {
		font-size: 12px;
		color: var(--muted);
		margin-top: 6px;
	}
	.bar {
		height: 6px;
		border-radius: 3px;
		background: var(--surface-2);
		margin-top: 8px;
		overflow: hidden;
	}
	.bar i {
		display: block;
		height: 100%;
		background: var(--accent);
		border-radius: 3px;
	}
	.coins {
		font-size: 12px;
		font-weight: 700;
		color: #c58a12;
		white-space: nowrap;
	}
</style>
