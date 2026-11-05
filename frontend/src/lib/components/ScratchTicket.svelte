<script lang="ts">
	import { onMount } from 'svelte';
	import { tone } from '#lib/fx.ts';

	/** Covers its content with a scratchable foil; `onScratched` fires once enough is cleared */
	let {
		label,
		onScratched,
		children
	}: { label: string; onScratched: () => void; children: import('svelte').Snippet } = $props();

	let canvas: HTMLCanvasElement;
	let host: HTMLDivElement;
	let gone = $state(false);

	export function reveal() {
		gone = true;
	}

	onMount(() => {
		let started = false;
		let done = false;
		const paint = () => {
			if (started || done) return;
			const r = host.getBoundingClientRect();
			if (!r.width) return;
			const dpr = devicePixelRatio || 1;
			canvas.width = Math.round(r.width * dpr);
			canvas.height = Math.round(r.height * dpr);
			const x = canvas.getContext('2d', { willReadFrequently: true })!;
			const g = x.createLinearGradient(0, 0, canvas.width, canvas.height);
			g.addColorStop(0, '#b9bdcc');
			g.addColorStop(0.45, '#eef0f6');
			g.addColorStop(0.55, '#d8dbe6');
			g.addColorStop(1, '#9ea3b6');
			x.fillStyle = g;
			x.fillRect(0, 0, canvas.width, canvas.height);
			for (let i = 0; i < 700; i++) {
				x.fillStyle = `rgba(255,255,255,${Math.random() * 0.6})`;
				x.fillRect(Math.random() * canvas.width, Math.random() * canvas.height, dpr, dpr);
			}
			x.fillStyle = '#4b5168';
			x.font = `800 ${16 * dpr}px "Bricolage Grotesque", sans-serif`;
			x.textAlign = 'center';
			x.textBaseline = 'middle';
			x.fillText(label, canvas.width / 2, canvas.height / 2);
			x.globalCompositeOperation = 'destination-out';
			x.lineCap = 'round';
			x.lineJoin = 'round';
			x.lineWidth = 36 * dpr;

			let last: [number, number] | null = null;
			let moves = 0;
			const pt = (e: PointerEvent): [number, number] => {
				const b = canvas.getBoundingClientRect();
				return [(e.clientX - b.left) * dpr, (e.clientY - b.top) * dpr];
			};
			const cleared = () => {
				const d = x.getImageData(0, 0, canvas.width, canvas.height).data;
				let c = 0;
				let t = 0;
				for (let i = 3; i < d.length; i += 40) {
					t++;
					if (d[i] === 0) c++;
				}
				return c / t;
			};
			const finish = () => {
				done = true;
				gone = true;
				onScratched();
			};
			canvas.onpointerdown = (e) => {
				if (done) return;
				started = true;
				canvas.setPointerCapture(e.pointerId);
				last = pt(e);
				x.beginPath();
				x.arc(last[0], last[1], 18 * dpr, 0, 7);
				x.fill();
			};
			canvas.onpointermove = (e) => {
				if (!last || done) return;
				const p = pt(e);
				x.beginPath();
				x.moveTo(last[0], last[1]);
				x.lineTo(p[0], p[1]);
				x.stroke();
				last = p;
				if (++moves % 6 === 0) {
					tone(240 + Math.random() * 180, 0.025, 'sawtooth', 0.012);
					if (cleared() > 0.42) finish();
				}
			};
			canvas.onpointerup = canvas.onpointercancel = () => {
				last = null;
				if (!done && cleared() > 0.42) finish();
			};
		};
		// Repaint when the ticket changes size, until someone starts scratching
		const ro = new ResizeObserver(paint);
		ro.observe(host);
		document.fonts?.ready.then(paint);
		return () => ro.disconnect();
	});
</script>

<div class="ticket" bind:this={host}>
	<div class="under">{@render children()}</div>
	<canvas bind:this={canvas} class:gone aria-hidden="true"></canvas>
</div>

<style>
	.ticket {
		position: relative;
		height: 96px;
		border-radius: 14px;
		background: repeating-linear-gradient(
			135deg,
			color-mix(in srgb, var(--legend) 16%, var(--surface)) 0 10px,
			color-mix(in srgb, var(--legend) 8%, var(--surface)) 10px 20px
		);
		border: 1.5px dashed color-mix(in srgb, var(--legend) 60%, transparent);
		overflow: hidden;
		-webkit-mask:
			radial-gradient(circle at 0 50%, transparent 10px, #000 10.5px) left/51% 100% no-repeat,
			radial-gradient(circle at 100% 50%, transparent 10px, #000 10.5px) right/51% 100% no-repeat;
		mask:
			radial-gradient(circle at 0 50%, transparent 10px, #000 10.5px) left/51% 100% no-repeat,
			radial-gradient(circle at 100% 50%, transparent 10px, #000 10.5px) right/51% 100% no-repeat;
	}
	.under {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
	}
	canvas {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		touch-action: none;
		cursor: crosshair;
		transition: opacity 0.5s;
	}
	canvas.gone {
		opacity: 0;
		pointer-events: none;
	}
</style>
