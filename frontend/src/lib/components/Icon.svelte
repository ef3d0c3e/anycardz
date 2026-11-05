<script lang="ts">
	const paths = {
		lobby: 'M4 9l1.5-5h13L20 9M4 9h16v11H4zM9 20v-6h6v6',
		binder: 'M6 3h11a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6zM6 3v18M10 8h5M10 12h5',
		open: 'M7 2h10l1 2.5-1 1V18l1 1.5-1 2.5H7l-1-2.5 1-1.5V5.5l-1-1zM7 7h10',
		forge: 'M12 3c1 3 5 5 5 10a5 5 0 0 1-10 0c0-2 1-3.5 2-4.5 0 2 1 3 2 3 0-3-1-5 1-8.5z',
		trade: 'M4 8h14l-3-3M20 16H6l3 3'
	} as const;
	let { name }: { name: keyof typeof paths } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true"><path d={paths[name]} /></svg>

<style>
	svg {
		width: 20px;
		height: 20px;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.8;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
</style>
