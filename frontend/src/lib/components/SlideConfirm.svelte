<script lang="ts">
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import { tone } from '#lib/fx.ts';

	let {
		label,
		onConfirm,
		tone: color = 'ok'
	}: { label: string; onConfirm: () => void; tone?: 'ok' | 'legend' } = $props();
	const i18n = getI18n();

	let root: HTMLDivElement;
	let knob: HTMLButtonElement;
	let x = $state(0);
	let dragging = $state(false);
	let x0 = 0;
	let max = 0;

	function down(e: PointerEvent) {
		knob.setPointerCapture(e.pointerId);
		x0 = e.clientX - x;
		max = root.clientWidth - knob.offsetWidth - 10;
		dragging = true;
	}
	function move(e: PointerEvent) {
		if (dragging) x = Math.max(0, Math.min(max, e.clientX - x0));
	}
	function up() {
		if (!dragging) return;
		dragging = false;
		if (x >= max * 0.9) {
			x = max;
			tone(880, 0.1, 'triangle', 0.05);
			setTimeout(() => {
				onConfirm();
				x = 0;
			}, 120);
		} else x = 0;
	}
</script>

<!-- Drag to the end to confirm: replaces "Are you sure?" dialogs -->
<div class="slide {color}" class:drag={dragging} bind:this={root}>
	<div class="fill" style:width="{x + 52}px"></div>
	<span class="label">{label}</span>
	<button
		type="button"
		class="knob"
		bind:this={knob}
		style:transform="translateX({x}px)"
		aria-label={i18n.t('inspect.slide_label', { label })}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={up}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				onConfirm();
			}
		}}>›</button
	>
</div>

<style>
	.slide {
		position: relative;
		height: 56px;
		border-radius: 16px;
		background: var(--surface-2);
		border: 1px solid var(--line);
		padding: 4px;
		overflow: hidden;
		user-select: none;
		-webkit-user-select: none;
		--c: var(--ok);
	}
	.slide.legend {
		--c: var(--legend);
	}
	.fill {
		position: absolute;
		left: 0;
		top: 0;
		bottom: 0;
		background: color-mix(in srgb, var(--c) 30%, transparent);
		border-radius: 16px;
		transition: width 0.35s;
	}
	.label {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		font-weight: 700;
		color: var(--muted);
		font-size: 14px;
		pointer-events: none;
		padding-left: 40px;
	}
	.knob {
		position: relative;
		width: 48px;
		height: 46px;
		border-radius: 12px;
		border: 0;
		background: var(--c);
		color: #fff;
		font: 800 20px/1 var(--display);
		touch-action: none;
		transition: transform 0.35s cubic-bezier(0.2, 1.3, 0.4, 1);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
	}
	.legend .knob {
		color: #2a1800;
	}
	.drag .knob,
	.drag .fill {
		transition: none;
	}
</style>
