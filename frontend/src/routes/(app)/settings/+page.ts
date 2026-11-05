import { api } from '#lib/api.ts';
import type { PageLoad } from './$types';

export interface SessionDto {
	id: number;
	user_agent: string;
	created_at: string;
	last_seen_at: string;
	current: boolean;
}

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('app:sessions');
	return { sessions: await api<SessionDto[]>('/account/sessions', {}, fetch) };
};
