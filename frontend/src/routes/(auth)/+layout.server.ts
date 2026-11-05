import { redirect } from '@sveltejs/kit';
import { api } from '#lib/api.ts';
import type { CollectionDto } from '#lib/types.ts';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ fetch }) => {
	// Already logged in: straight to the lobby
	const me = await fetch('/api/me').catch(() => null);
	if (me?.ok) redirect(303, '/');
	// Featured cards for the decorative fan; the page works without them
	const collections = await api<CollectionDto[]>('/collections', {}, fetch).catch(() => []);
	return { collections };
};
