<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getCatalog, getSession } from '#lib/session.svelte.ts';
	import { api } from '#lib/api.ts';
	import { missionName, WHERE } from '#lib/missions.ts';
	import type { MissionsDto } from '#lib/types.ts';
	import Pack from './Pack.svelte';

	/** Today's three missions, with progress, rewards and the bonus pack */
	const i18n = getI18n();
	const session = getSession();
	const catalog = getCatalog()!;
	const m = $derived(session.missions);
	const done = $derived(m.items.filter((x) => x.done).length);

	// Progress moves without a notification: fresh numbers on arrival
	onMount(() => {
		api<MissionsDto>('/missions')
			.then((fresh) => (session.missions = fresh))
			.catch(() => {});
	});

	let left = $state('');
	function tick() {
		const s = Math.max(0, Math.floor((new Date(m.resets_at).getTime() - Date.now()) / 1000));
		left = [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60]
			.map((v) => String(v).padStart(2, '0'))
			.join(':');
		// A new day: new missions
		if (s === 0)
			api<MissionsDto>('/missions')
				.then((fresh) => (session.missions = fresh))
				.catch(() => {});
	}
	tick();
	const timer = setInterval(tick, 1000);
	onDestroy(() => clearInterval(timer));
</script>

<section class="panel missions" aria-labelledby="missions-title">
	<div class="head">
		<div>
			<p class="eyebrow">{i18n.t('mission.title')}</p>
			<h2 id="missions-title">
				{i18n.t('mission.heading', { done, total: m.items.length })}
			</h2>
		</div>
		<p class="next">{i18n.t('mission.next', { time: left })}</p>
	</div>
	<ul>
		{#each m.items as x (x.key)}
			<li class:done={x.done}>
				<span class="check" aria-hidden="true">{x.done ? '✓' : ''}</span>
				<div class="what">
					<span class="name">{missionName(i18n, x.key, x.goal)}</span>
					<span
						class="bar"
						role="progressbar"
						aria-valuemin={0}
						aria-valuemax={x.goal}
						aria-valuenow={x.progress}
						><span style:width="{(100 * x.progress) / x.goal}%"></span></span
					>
				</div>
				<span class="count">{x.progress}/{x.goal}</span>
				<span class="reward" aria-label={i18n.t('ach.coins', { n: x.coins })}
					><span class="coin" aria-hidden="true">c</span>{x.coins}</span
				>
				{#if x.done}
					<span class="sr-label">{i18n.t('mission.done_label')}</span>
				{:else}
					<a class="go" href={WHERE[x.key] ?? '/'}>{i18n.t('mission.go')}</a>
				{/if}
			</li>
		{/each}
	</ul>
	<div class="bonus" class:got={m.bonus}>
		<span class="mini" aria-hidden="true">
			{#if catalog.free}<Pack collection={catalog.free} />{/if}
		</span>
		<span>{m.bonus ? i18n.t('mission.bonus_given') : i18n.t('mission.bonus')}</span>
		{#if m.bonus}<span class="check" aria-hidden="true">✓</span>{/if}
	</div>
</section>

<style>
	.missions {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 20px;
		padding: 18px;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.head {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: 10px;
		flex-wrap: wrap;
	}
	h2 {
		font-size: 22px;
	}
	.next {
		color: var(--muted);
		font-size: 13px;
		font-variant-numeric: tabular-nums;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	li {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border-radius: 14px;
		background: var(--surface-2);
	}
	li.done {
		opacity: 0.7;
	}
	.check {
		width: 22px;
		height: 22px;
		flex: none;
		border-radius: 50%;
		border: 2px solid var(--line);
		display: grid;
		place-items: center;
		font-size: 12px;
		font-weight: 800;
		color: #fff;
	}
	li.done .check,
	.bonus.got .check {
		background: var(--ok);
		border-color: var(--ok);
	}
	.what {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.name {
		font-weight: 600;
		font-size: 14px;
	}
	.bar {
		height: 6px;
		border-radius: 3px;
		background: var(--line);
		overflow: hidden;
	}
	.bar span {
		display: block;
		height: 100%;
		background: var(--accent);
		border-radius: 3px;
		transition: width 0.4s;
	}
	li.done .bar span {
		background: var(--ok);
	}
	.count {
		font: 600 12.5px var(--mono);
		color: var(--muted);
	}
	.reward {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		font: 700 13px var(--mono);
	}
	.reward .coin {
		width: 14px;
		height: 14px;
		font-size: 8px;
	}
	.go {
		font-size: 12.5px;
		font-weight: 700;
		color: var(--accent);
		text-decoration: none;
		padding: 4px 8px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
	}
	.sr-label {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}
	.bonus {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 13.5px;
		font-weight: 600;
		padding: 8px 12px;
		border: 1.5px dashed color-mix(in srgb, #b05cff 50%, var(--line));
		border-radius: 14px;
	}
	.bonus.got {
		border-style: solid;
	}
	.mini {
		width: 22px;
		flex: none;
	}
</style>
