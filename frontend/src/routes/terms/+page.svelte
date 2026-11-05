<script lang="ts">
	import type { PageProps } from './$types';
	import LegalPage from '#lib/components/LegalPage.svelte';
	let { data }: PageProps = $props();
</script>

<LegalPage which="terms" info={data.info} />
