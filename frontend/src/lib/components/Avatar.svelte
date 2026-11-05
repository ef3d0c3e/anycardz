<script lang="ts">
	/** A player's picture, or their initial on a colored disc */
	let {
		name,
		src = null,
		size = 28,
		online
	}: { name: string; src?: string | null; size?: number; online?: boolean } = $props();
	// A stable color per name, so a player keeps theirs everywhere
	const hue = $derived([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
</script>

<span class="avatar" style:--s="{size}px" style:--h={hue} aria-hidden="true">
	{#if src}<img {src} alt="" loading="lazy" />{:else}{name[0]?.toUpperCase()}{/if}
	{#if online !== undefined}<i class:on={online}></i>{/if}
</span>

<style>
	.avatar {
		position: relative;
		width: var(--s);
		height: var(--s);
		border-radius: 50%;
		display: grid;
		place-items: center;
		flex: none;
		background: hsl(var(--h) 55% 45%);
		color: #fff;
		font: 800 calc(var(--s) * 0.42) / 1 var(--display);
	}
	img {
		width: 100%;
		height: 100%;
		border-radius: 50%;
		object-fit: cover;
	}
	i {
		position: absolute;
		right: 0;
		bottom: 0;
		width: max(8px, calc(var(--s) * 0.26));
		height: max(8px, calc(var(--s) * 0.26));
		border-radius: 50%;
		background: var(--muted);
		box-shadow: 0 0 0 2px var(--surface);
	}
	i.on {
		background: var(--ok);
	}
</style>
