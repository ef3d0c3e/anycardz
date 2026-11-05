import { api } from '#lib/api.ts';
import type { BinderDto, CollectionDto } from '#lib/types.ts';
import { defaultSet } from '#lib/binder/pick.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url, parent }) => {
	const { collections } = await parent();
	const wanted = collections.find((c) => c.slug === url.searchParams.get('set'))?.slug;
	// A link names its set; otherwise the binder opens where the player is closest to a full set
	const binderCall = api<BinderDto>('/binder', {}, fetch);
	const slug =
		wanted ?? defaultSet(collections, (await binderCall).collections) ?? collections[0].slug;
	const [binder, collection] = await Promise.all([
		binderCall,
		api<CollectionDto>(`/collections/${slug}`, {}, fetch)
	]);
	return { binder, collection };
};
