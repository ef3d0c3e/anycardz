<script lang="ts">
	let { value, locale = 'en-US' }: { value: number; locale?: string } = $props();
	const chars = $derived([...value.toLocaleString(locale)]);
</script>

<!-- Rolling digits: each digit is a column of 0-9 that slides into place -->
<span class="odo" aria-hidden="true">
	{#each chars as ch, i (chars.length - i)}
		{#if /\d/.test(ch)}
			<span class="d"
				><span class="strip" style:transform="translateY({-Number(ch)}em)">
					{#each '0123456789' as n}<span>{n}</span>{/each}
				</span></span
			>
		{:else}
			<span>{ch === ' ' || ch === ' ' ? ' ' : ch}</span>
		{/if}
	{/each}
</span>

<style>
	.odo {
		display: inline-flex;
		height: 1em;
		line-height: 1em;
	}
	.d {
		display: inline-block;
		height: 1em;
		overflow: hidden;
	}
	.strip {
		display: flex;
		flex-direction: column;
		transition: transform 0.9s cubic-bezier(0.2, 0.9, 0.25, 1);
	}
	.strip span {
		height: 1em;
		line-height: 1em;
	}
</style>
