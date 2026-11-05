<script lang="ts">
	import Card from './Card.svelte';
	import { getI18n } from '#lib/i18n/index.svelte.ts';
	import type { CollectionDto } from '#lib/types.ts';

	let { collection, style = '' }: { collection: CollectionDto; style?: string } = $props();
	const i18n = getI18n();
</script>

<!-- A booster: the set's featured card behind a glossy wrapper -->
<div
	class="pack"
	class:free={collection.free}
	style:--cp={collection.primary}
	style:--cs={collection.secondary}
	{style}
>
	<div class="pk">
		<span class="crimp t"></span>
		<div class="pk-head">
			<span class="pk-brand">AnyCardz</span>
			<span class="pk-name">{collection.title}</span>
		</div>
		<div class="pk-feature">
			<div class="pk-card"><Card card={collection.featured} foil="foil3" /></div>
		</div>
		<span class="pk-meta"
			>{collection.free ? i18n.t('free.meta') : i18n.t('pack.meta', { n: collection.total })}</span
		>
		<span class="pk-gloss"></span>
		<span class="crimp b"></span>
	</div>
</div>
