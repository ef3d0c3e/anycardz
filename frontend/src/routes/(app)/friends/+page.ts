import { api } from '#lib/api.ts';
import type { FriendsDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('app:friends');
	return { friends: await api<FriendsDto>('/friends', {}, fetch) };
};
