import { redirect } from '@sveltejs/kit';
import { api, ApiError } from '#lib/api.ts';
import type { CollectionDto, MeDto } from '#lib/types.ts';
import type { LayoutLoad } from './$types';

// Universal load: runs on the server for the first page, then in the browser
export const load: LayoutLoad = async ({ fetch, depends }) => {
	depends('app:catalog');
	try {
		const [me, collections] = await Promise.all([
			api<MeDto>('/me', {}, fetch),
			api<CollectionDto[]>('/collections', {}, fetch)
		]);
		return { me, collections };
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) redirect(303, '/login');
		throw e;
	}
};
