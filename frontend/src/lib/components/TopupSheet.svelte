<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { getTopup } from '#lib/topup.svelte.ts';
	import { api, post } from '#lib/api.ts';
	import { arp, flyCoins } from '#lib/fx.ts';

	interface Pack {
		id: string;
		coins: number;
		price_cents: number;
	}
	const i18n = getI18n();
	const session = getSession();
	const toasts = getToasts();
	const topup = getTopup();

	let packs = $state<Pack[]>([]);
	let chosen = $state<string>('');
	let busy = $state(false);
	let dialog = $state<HTMLDivElement>();

	const need = $derived(topup.request.need ?? 0);
	// Coins per cent: the pack that gives the most for the money is flagged
	const bestValue = $derived(
		packs.reduce<Pack | null>(
			(b, p) => (!b || p.coins / p.price_cents > b.coins / b.price_cents ? p : b),
			null
		)?.id
	);
	const pick = $derived(packs.find((p) => p.id === chosen));
	const price = (cents: number) =>
		new Intl.NumberFormat(i18n.lang === 'fr' ? 'fr-FR' : 'en-US', {
			style: 'currency',
			currency: 'EUR'
		}).format(cents / 100);

	$effect(() => {
		if (!topup.open) return;
		const preselect = (list: Pack[]) => {
			// The smallest pack that covers the shortfall, else a middle one
			chosen = (need ? list.find((p) => p.coins >= need) : list[1])?.id ?? list.at(-1)?.id ?? '';
		};
		if (packs.length) preselect(packs);
		else
			api<{ packs: Pack[] }>('/shop/topups')
				.then((r) => {
					packs = r.packs;
					preselect(r.packs);
				})
				.catch((e) => toasts.show(i18n.error(e), 'warn'));
		queueMicrotask(() => dialog?.focus());
	});

	async function confirm() {
		if (!pick || busy) return;
		busy = true;
		try {
			const res = await post<{ coins: number; added: number }>('/shop/topup', { pack: pick.id });
			arp([523, 784, 1046]);
			const from = dialog?.querySelector('[aria-pressed="true"]');
			const then = topup.request.then;
			topup.close();
			if (from) await flyCoins(from, 14);
			session.coins = res.coins;
			toasts.show(i18n.t('topup.added', { n: res.added }));
			await then?.();
		} catch (e) {
			toasts.show(i18n.error(e), 'warn');
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => topup.open && e.key === 'Escape' && topup.close()} />

{#if topup.open}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="modal" onclick={(e) => e.target === e.currentTarget && topup.close()}>
		<div
			class="sheet"
			role="dialog"
			aria-modal="true"
			aria-labelledby="topup-title"
			tabindex="-1"
			bind:this={dialog}
		>
			<button type="button" class="x" onclick={topup.close} aria-label={i18n.t('common.close')}
				>✕</button
			>
			<div class="head">
				<p class="eyebrow">{i18n.t('topup.balance', { n: session.coins })}</p>
				<h2 id="topup-title">{i18n.t('topup.title')}</h2>
				<p class="lead">
					{need
						? i18n.t('topup.need', { n: need, reason: topup.request.reason ?? '' })
						: i18n.t('topup.lead')}
				</p>
			</div>
			<div class="packs" role="group" aria-label={i18n.t('topup.title')}>
				{#each packs as p (p.id)}
					{@const covers = !need || p.coins >= need}
					<button
						type="button"
						class="coinpack"
						class:short={!covers}
						aria-pressed={chosen === p.id}
						onclick={() => (chosen = p.id)}
					>
						{#if p.id === bestValue}<span class="tag">{i18n.t('topup.best')}</span>{/if}
						<span class="stack" aria-hidden="true">
							{#each Array(Math.min(5, Math.ceil(Math.log10(p.coins)))) as _, i}<span
									class="coin"
									style:--i={i}>c</span
								>{/each}
						</span>
						<b class="mono">{i18n.num(p.coins)}</b>
						<span class="price">{price(p.price_cents)}</span>
					</button>
				{/each}
			</div>
			<p class="demo">{i18n.t('topup.demo')}</p>
			<button type="button" class="btn primary go" disabled={!pick || busy} onclick={confirm}>
				{pick
					? i18n.t(topup.request.then ? 'topup.get_and_buy' : 'topup.get', { n: pick.coins })
					: i18n.t('auth.working')}
			</button>
		</div>
	</div>
{/if}

<style>
	.modal {
		position: fixed;
		inset: 0;
		z-index: 70;
		display: grid;
		place-items: end center;
		background: rgba(8, 10, 25, 0.6);
		backdrop-filter: blur(6px);
		animation: fade 0.2s;
	}
	@media (min-width: 640px) {
		.modal {
			place-items: center;
			padding: 16px;
		}
	}
	.sheet {
		position: relative;
		width: min(560px, 100%);
		max-height: 100%;
		overflow: auto;
		background: var(--surface);
		border-radius: 24px 24px 0 0;
		padding: 22px max(18px, env(safe-area-inset-right, 0px))
			calc(18px + env(safe-area-inset-bottom, 0px)) max(18px, env(safe-area-inset-left, 0px));
		display: flex;
		flex-direction: column;
		gap: 16px;
		box-shadow: 0 30px 80px -20px rgba(0, 0, 0, 0.6);
		animation: up 0.3s cubic-bezier(0.2, 1.2, 0.4, 1);
		outline: none;
	}
	@media (min-width: 640px) {
		.sheet {
			border-radius: 24px;
			padding: 26px;
		}
	}
	h2 {
		font-size: 28px;
		margin-top: 4px;
	}
	.lead {
		color: var(--muted);
		margin-top: 6px;
	}
	.packs {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		gap: 10px;
	}
	.coinpack {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 16px 10px 12px;
		border-radius: 16px;
		border: 1.5px solid var(--line);
		background: var(--surface-2);
		transition:
			transform 0.15s,
			border-color 0.15s;
	}
	.coinpack:hover {
		transform: translateY(-2px);
	}
	.coinpack[aria-pressed='true'] {
		border-color: var(--legend);
		background: color-mix(in srgb, var(--legend) 12%, var(--surface));
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--legend) 25%, transparent);
	}
	/* Packs too small for the shortfall stay pickable, just quieter */
	.coinpack.short:not([aria-pressed='true']) {
		opacity: 0.55;
	}
	.coinpack b {
		font-size: 20px;
	}
	.price {
		color: var(--muted);
		font-size: 13px;
		font-weight: 600;
	}
	.stack {
		display: flex;
		height: 26px;
		align-items: flex-end;
	}
	.stack .coin {
		width: 22px;
		height: 22px;
		margin-left: -9px;
		translate: 0 calc(var(--i) * -2px);
	}
	.stack .coin:first-child {
		margin-left: 0;
	}
	.tag {
		position: absolute;
		top: -9px;
		font: 700 10px/1 var(--mono);
		letter-spacing: 0.06em;
		text-transform: uppercase;
		background: var(--ok);
		color: #fff;
		padding: 4px 7px;
		border-radius: 999px;
	}
	.demo {
		font-size: 12.5px;
		color: var(--muted);
		padding: 10px 12px;
		border-radius: 10px;
		background: var(--surface-2);
		border: 1px dashed var(--line);
	}
	.go {
		padding: 14px;
		font-size: 15px;
	}
	.x {
		position: absolute;
		top: 12px;
		right: 12px;
		width: 36px;
		height: 36px;
		border-radius: 10px;
		border: 0;
		background: var(--surface-2);
		font-size: 18px;
	}
</style>
