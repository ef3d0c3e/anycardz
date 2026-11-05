import { api } from '#lib/api.ts';
import type { BinderDto, CollectionDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url, parent }) => {
	const { collections } = await parent();
	const wanted = url.searchParams.get('set');
	const slug = collections.find((c) => c.slug === wanted)?.slug ?? collections[0].slug;
	const [binder, collection] = await Promise.all([
		api<BinderDto>('/binder', {}, fetch),
		api<CollectionDto>(`/collections/${slug}`, {}, fetch)
	]);
	return { binder, collection };
};
