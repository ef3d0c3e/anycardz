<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { post } from '#lib/api.ts';
	import { arp, burst, tone } from '#lib/fx.ts';
	import type { ScratchDto, Symbol } from '#lib/types.ts';
	import ScratchTicket from './ScratchTicket.svelte';

	/**
	 * One scratch ticket: three slots to scratch. The server draws them at the
	 * first touch; the prize shows once all three are uncovered.
	 */
	interface Props {
		/** The player goes on: the next ticket, or the packs */
		onNext: () => void;
		/** Packs first, the tickets wait */
		onLater: () => void;
	}
	let { onNext, onLater }: Props = $props();
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();

	const ICON: Record<Symbol, string> = {
		gem: '💎',
		coin: '🪙',
		pack: '🎁',
		sparkle: '✨',
		star: '⭐'
	};
	let result = $state<ScratchDto | null>(null);
	let playing: Promise<ScratchDto | null> | null = null;
	let uncovered = $state([false, false, false]);
	let slots = $state<ScratchTicket[]>([]);
	let card = $state<HTMLElement>();
	const done = $derived(!!result && uncovered.every(Boolean));
	const left = $derived(result ? result.tickets.count : session.tickets.count);

	/** Asks for the draw once, whatever slot is touched first */
	function play() {
		playing ??= post<ScratchDto>('/tickets/scratch')
			.then((r) => (result = r))
			.catch((e) => {
				toasts.show(i18n.error(e), 'warn');
				playing = null;
				return null;
			});
		return playing;
	}

	async function uncover(i: number) {
		if (!(await play())) return;
		uncovered[i] = true;
		tone(520 + i * 120, 0.08, 'triangle', 0.04);
		if (uncovered.every(Boolean)) finish();
	}

	async function revealAll() {
		if (!(await play())) return;
		slots.forEach((s) => s?.reveal());
		uncovered = [true, true, true];
		finish();
	}

	/** The prize shows: the balance, pile and tickets follow */
	function finish() {
		const r = result!;
		session.coins = r.coins;
		session.pile = r.pile;
		session.tickets = r.tickets;
		if (r.prize !== 'nothing') {
			arp(r.prize === 'pair' ? [660, 880] : [660, 880, 1100, 1320], 0.08, 'triangle', 0.05);
			if (card && r.prize !== 'pair') burst(card, '#ffcf5c', 40);
		}
	}

	const time = (iso: string) =>
		new Date(iso).toLocaleTimeString(i18n.lang, { hour: '2-digit', minute: '2-digit' });
</script>

<section class="scratch" bind:this={card} aria-labelledby="scratch-title">
	<div class="head">
		<!-- Tickets in hand, this one included -->
		<p class="eyebrow">{i18n.t('scratch.eyebrow', { n: result ? left + 1 : left })}</p>
		<h1 id="scratch-title">{i18n.t('scratch.title')}</h1>
		<p class="rules">{i18n.t('scratch.rules')}</p>
	</div>

	<div class="slots">
		{#each [0, 1, 2] as i (i)}
			<div class="slot" class:shown={uncovered[i]}>
				<ScratchTicket
					bind:this={slots[i]}
					label={i18n.t('scratch.slot')}
					onStart={play}
					onScratched={() => uncover(i)}
				>
					{#if result}
						<span class="symbol" aria-label={i18n.t(`scratch.symbol.${result.slots[i]}`)}
							>{ICON[result.slots[i]]}</span
						>
					{/if}
				</ScratchTicket>
			</div>
		{/each}
	</div>

	{#if done && result}
		<p class="prize" class:win={result.prize !== 'nothing'} role="status">
			{#if result.prize === 'boost' && result.tickets.boost_until}
				{i18n.t('scratch.prize.boost', { time: time(result.tickets.boost_until) })}
			{:else}
				{i18n.t(`scratch.prize.${result.prize}`, { n: result.coins_won })}
			{/if}
		</p>
		<div class="row">
			{#if left > 0}
				<button type="button" class="btn primary" onclick={onNext}
					>{i18n.t('scratch.next', { n: left })}</button
				>
			{/if}
			{#if session.pile.length}
				<button type="button" class="btn" class:primary={left === 0} onclick={onLater}
					>{i18n.t('scratch.to_packs', { n: session.pile.length })}</button
				>
			{:else if left === 0}
				<a class="btn primary" href="/">{i18n.t('open.buy_more')}</a>
			{/if}
		</div>
		{#if result.tickets.next_at}
			<p class="refill">{i18n.t('scratch.refill', { time: time(result.tickets.next_at) })}</p>
		{/if}
	{:else}
		<p class="hint">
			{i18n.t('scratch.hint')}
			<button type="button" class="linkish" onclick={revealAll}>{i18n.t('scratch.reveal')}</button>
		</p>
		{#if session.pile.length && !result}
			<button type="button" class="later linkish" onclick={onLater}
				>{i18n.t('scratch.later', { n: session.pile.length })}</button
			>
		{/if}
	{/if}
</section>

<style>
	.scratch {
		position: relative;
		z-index: 2;
		width: min(460px, 100%);
		margin: auto 0;
		display: flex;
		flex-direction: column;
		gap: 14px;
		align-items: stretch;
		text-align: center;
		padding: 20px 18px;
		border-radius: 22px;
		background:
			linear-gradient(160deg, rgba(255, 207, 92, 0.18), rgba(255, 79, 163, 0.12)),
			rgba(16, 20, 46, 0.85);
		border: 1.5px dashed rgba(255, 207, 92, 0.55);
		box-shadow: 0 20px 60px -20px rgba(0, 0, 0, 0.6);
		/* The slots in gold */
		--legend: #ffcf5c;
		--surface: #1a1f45;
		--muted: rgba(238, 240, 255, 0.6);
	}
	.head h1 {
		font-size: 28px;
	}
	.eyebrow {
		color: rgba(238, 240, 255, 0.6);
	}
	.rules,
	.hint,
	.refill {
		font-size: 13px;
		color: rgba(238, 240, 255, 0.7);
	}
	.slots {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 10px;
		--ticket-h: 104px;
	}
	.symbol {
		font-size: 44px;
		line-height: 1;
	}
	.slot.shown .symbol {
		animation: pop 0.4s cubic-bezier(0.2, 1.6, 0.4, 1);
	}
	.prize {
		font: 800 20px/1.25 var(--display);
		color: rgba(238, 240, 255, 0.75);
	}
	.prize.win {
		color: #ffcf5c;
	}
	.row {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
	}
	.row .btn {
		flex: 1;
		background: transparent;
		color: var(--felt-ink);
		border-color: rgba(238, 240, 255, 0.3);
	}
	.row .btn.primary {
		background: var(--felt-ink);
		color: var(--felt);
		border-color: transparent;
	}
	.linkish {
		color: inherit;
		text-decoration: underline;
		background: none;
		border: 0;
		padding: 0;
		font: inherit;
		cursor: pointer;
	}
	.later {
		align-self: center;
		font-size: 13px;
		color: rgba(238, 240, 255, 0.7);
	}
</style>
