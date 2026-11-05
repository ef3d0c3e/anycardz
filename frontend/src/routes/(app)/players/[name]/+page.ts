import { error } from '@sveltejs/kit';
import { api, ApiError } from '#lib/api.ts';
import type { ProfileDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params }) => {
	try {
		return {
			profile: await api<ProfileDto>(`/players/${encodeURIComponent(params.name)}`, {}, fetch)
		};
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'unknown_player');
		throw e;
	}
};
