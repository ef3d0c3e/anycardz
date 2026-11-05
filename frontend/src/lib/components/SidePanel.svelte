<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { getSession, getToasts } from '#lib/session.svelte.ts';
	import { ago, getLive } from '#lib/live.svelte.ts';
	import { artUrl, post } from '#lib/api.ts';
	import type { ChatEvent } from '#lib/types.ts';
	import Card from './Card.svelte';
	import Avatar from './Avatar.svelte';

	/** Live pulls and the shoutbox: a column on desktop, a sheet elsewhere */
	let { onClose }: { onClose?: () => void } = $props();
	const i18n = getI18n();
	const live = getLive();
	const session = getSession();
	const toasts = getToasts();

	let tab = $state<'pulls' | 'chat'>('pulls');
	let draft = $state('');
	let sending = $state(false);
	let list = $state<HTMLElement>();
	// Re-renders relative times every 20 s
	let now = $state(Date.now());
	onMount(() => {
		const t = setInterval(() => (now = Date.now()), 20_000);
		return () => clearInterval(t);
	});

	$effect(() => {
		if (tab === 'chat') live.unread = 0;
	});
	// New messages scroll into view when the reader is already at the bottom
	$effect(() => {
		void live.messages.length;
		if (tab !== 'chat' || !list) return;
		const nearBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 80;
		if (nearBottom) tick().then(() => list && (list.scrollTop = list.scrollHeight));
	});

	async function send(e: SubmitEvent) {
		e.preventDefault();
		if (!draft.trim() || sending) return;
		sending = true;
		try {
			live.sent(await post<ChatEvent>('/shoutbox', { body: draft }));
			draft = '';
			await tick();
			if (list) list.scrollTop = list.scrollHeight;
		} catch (err) {
			toasts.show(i18n.error(err), 'warn');
		} finally {
			sending = false;
		}
	}
	const who = (u: string) => (u === session.username ? i18n.t('live.you') : u);
	const verb = { pack: 'live.pulled', tradeup: 'live.forged', roulette: 'live.foiled' } as const;
</script>

<div class="panel">
	<div class="head">
		<div class="seg" role="group" aria-label={i18n.t('live.panel')}>
			<button type="button" aria-pressed={tab === 'pulls'} onclick={() => (tab = 'pulls')}
				>{i18n.t('live.pulls')}</button
			>
			<button type="button" aria-pressed={tab === 'chat'} onclick={() => (tab = 'chat')}>
				{i18n.t('live.chat')}{#if live.unread && tab !== 'chat'}<span
						class="dot"
						aria-label={i18n.t('live.unread', { n: live.unread })}>{live.unread}</span
					>{/if}
			</button>
		</div>
		{#if !live.connected}<span class="offline" role="status">{i18n.t('live.offline')}</span>{/if}
		{#if onClose}<button
				type="button"
				class="x"
				onclick={onClose}
				aria-label={i18n.t('common.close')}>✕</button
			>{/if}
	</div>

	{#if tab === 'pulls'}
		<ul class="pulls" aria-live="polite">
			{#each live.pulls as p (`${p.at}-${p.user}-${p.card}`)}
				{@const c = live.cards[p.card]}
				<li
					class="pull r-tag-{p.rarity}"
					class:foil={p.foil}
					class:legend={p.rarity === 'legendary'}
				>
					<span class="thumb"><img src={artUrl(p.card)} alt="" loading="lazy" /></span>
					<p>
						{i18n.t(verb[p.source], { user: who(p.user), card: c?.title ?? p.card })}
						<small
							>{i18n.t(`card.${p.rarity}`)}{p.foil ? ` · ${i18n.t('card.foil')}` : ''} ·
							<time datetime={p.at}>{ago(p.at, i18n.lang, now)}</time></small
						>
					</p>
				</li>
			{:else}
				<li class="empty">{i18n.t('live.no_pulls')}</li>
			{/each}
		</ul>
	{:else}
		<div class="chat">
			<ul class="msgs" bind:this={list} aria-live="polite">
				{#each live.messages as m (m.id)}
					<li class="msg" class:me={m.user === session.username}>
						<Avatar name={m.user} size={28} />
						<div class="bubble">
							<p class="meta">
								<a class="user" href="/players/{encodeURIComponent(m.user)}">{who(m.user)}</a>
								<time datetime={m.at}>{ago(m.at, i18n.lang, now)}</time>
							</p>
							{#if m.body}<p class="body">{m.body}</p>{/if}
							{#if m.card && live.cards[m.card]}
								{@const c = live.cards[m.card]}
								<div class="brag">
									<div class="mini"><Card card={c} foil={m.foil} /></div>
									<div>
										<b>{c.title}</b><small
											>{i18n.t(`card.${c.rarity}`)}{m.foil
												? ` · ${i18n.t('card.foil')}`
												: ''}</small
										>
									</div>
								</div>
							{/if}
						</div>
					</li>
				{:else}
					<li class="empty">{i18n.t('live.no_messages')}</li>
				{/each}
			</ul>
			<form onsubmit={send}>
				<input
					id="shout-input"
					bind:value={draft}
					maxlength="300"
					placeholder={i18n.t('live.placeholder')}
					aria-label={i18n.t('live.placeholder')}
					autocomplete="off"
				/>
				<button class="btn sm accent" type="submit" disabled={sending || !draft.trim()}
					>{i18n.t('live.send')}</button
				>
			</form>
		</div>
	{/if}
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		background: var(--surface);
	}
	.head {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 12px;
		border-bottom: 1px solid var(--line);
	}
	.head .seg {
		flex: 1;
	}
	.head .seg button {
		flex: 1;
		display: inline-flex;
		justify-content: center;
		align-items: center;
		gap: 6px;
	}
	.dot {
		min-width: 18px;
		height: 18px;
		border-radius: 9px;
		padding: 0 5px;
		background: var(--danger);
		color: #fff;
		font: 700 11px/18px var(--mono);
	}
	.offline {
		font-size: 11px;
		color: var(--warn);
	}
	.x {
		border: 0;
		background: var(--surface-2);
		width: 34px;
		height: 34px;
		border-radius: 10px;
		font-size: 16px;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 6px 8px;
		overflow: auto;
		flex: 1;
		min-height: 0;
	}
	.empty {
		color: var(--muted);
		font-size: 13px;
		padding: 18px 8px;
		text-align: center;
	}
	.pull {
		display: grid;
		grid-template-columns: 40px minmax(0, 1fr);
		gap: 10px;
		align-items: center;
		padding: 8px;
		border-radius: 10px;
		animation: slidein 0.45s ease-out;
	}
	@keyframes slidein {
		from {
			transform: translateX(-14px);
			opacity: 0;
		}
	}
	.pull.legend {
		background: color-mix(in srgb, var(--legend) 10%, transparent);
	}
	.thumb {
		width: 40px;
		height: 40px;
		border-radius: 9px;
		overflow: hidden;
		position: relative;
	}
	/* The rarity frame, drawn over the art */
	.thumb::before {
		content: '';
		position: absolute;
		inset: 0;
		border-radius: inherit;
		box-shadow: inset 0 0 0 2px var(--rc);
		z-index: 1;
		pointer-events: none;
	}
	.thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.pull.foil .thumb::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(
			115deg,
			transparent 25%,
			rgba(255, 120, 200, 0.6),
			rgba(120, 255, 210, 0.6),
			rgba(120, 180, 255, 0.6),
			transparent 75%
		);
		mix-blend-mode: color-dodge;
	}
	.pull p {
		font-size: 13px;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.pull small,
	.brag small {
		display: block;
		color: var(--muted);
		font-size: 11.5px;
	}
	.chat {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 0;
	}
	.msgs {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 10px 12px;
	}
	.msg {
		display: grid;
		grid-template-columns: 28px minmax(0, 1fr);
		gap: 8px;
	}
	.user {
		color: var(--ink);
		font-weight: 700;
		text-decoration: none;
	}
	.user:hover {
		text-decoration: underline;
	}
	.meta {
		font-size: 12.5px;
	}
	.meta time {
		color: var(--muted);
		font-size: 11px;
		margin-left: 4px;
	}
	.body {
		font-size: 13.5px;
		overflow-wrap: anywhere;
	}
	.brag {
		margin-top: 6px;
		display: flex;
		gap: 10px;
		align-items: center;
		padding: 8px;
		border-radius: 12px;
		background: var(--surface-2);
		border: 1px solid var(--line);
		max-width: 260px;
	}
	.mini {
		width: 52px;
		flex: none;
	}
	form {
		display: flex;
		gap: 8px;
		padding: 10px 12px calc(10px + env(safe-area-inset-bottom, 0px));
		border-top: 1px solid var(--line);
	}
	input {
		flex: 1;
		min-width: 0;
		border: 1px solid var(--line);
		background: var(--surface-2);
		border-radius: 10px;
		padding: 9px 12px;
		font-size: 15px;
	}
</style>
