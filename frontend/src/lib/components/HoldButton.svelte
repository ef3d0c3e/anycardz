<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		ms?: number;
		disabled?: boolean;
		class?: string;
		fill?: string;
		/** Accessible name, when the visible text is short */
		label?: string;
		onDone: () => void;
		onProgress?: (p: number) => void;
		onCancel?: () => void;
		/** Released almost at once: the player did not know it needs a hold */
		onTooShort?: () => void;
		children: Snippet;
	}
	let {
		ms = 650,
		disabled = false,
		class: cls = '',
		fill,
		label,
		onDone,
		onProgress,
		onCancel,
		onTooShort,
		children
	}: Props = $props();

	let p = $state(0);
	let holding = $state(false);
	let raf = 0;
	let t0 = 0;

	function start(e: Event) {
		if (disabled || holding) return;
		if (e.cancelable) e.preventDefault();
		holding = true;
		t0 = performance.now();
		const step = (t: number) => {
			p = Math.min(1, (t - t0) / ms);
			onProgress?.(p);
			if (p < 1) raf = requestAnimationFrame(step);
			else {
				holding = false;
				p = 0;
				onDone();
			}
		};
		raf = requestAnimationFrame(step);
	}
	function end() {
		if (!holding) return;
		holding = false;
		cancelAnimationFrame(raf);
		p = 0;
		onCancel?.();
		if (performance.now() - t0 < 260) onTooShort?.();
	}
	const key = (e: KeyboardEvent) => e.key === ' ' || e.key === 'Enter';
</script>

<button
	type="button"
	class="hold {cls}"
	aria-label={label}
	class:holding
	{disabled}
	style:--p={p}
	style:--fill={fill}
	onpointerdown={start}
	onpointerup={end}
	onpointerleave={end}
	onpointercancel={end}
	onkeydown={(e) => key(e) && !e.repeat && start(e)}
	onkeyup={(e) => key(e) && end()}
	oncontextmenu={(e) => e.preventDefault()}
>
	{@render children()}
</button>
