import { api } from '#lib/api.ts';
import type { MeDto, ProfileDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

// Loaded before the page shows: an answer arriving later would overwrite what the player typed
export const load: PageLoad = async ({ fetch, parent }) => {
	const { me } = (await parent()) as { me: MeDto };
	const profile = await api<ProfileDto>(`/players/${encodeURIComponent(me.username)}`, {}, fetch);
	return { bio: profile.bio };
};
