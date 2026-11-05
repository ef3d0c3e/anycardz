import { api } from '#lib/api.ts';
import type { LegalInfo } from '#lib/legal.ts';
import type { PageLoad } from './$types';

// Public: no session needed. Without the API the text still shows, with a
// generic operator.
export const load: PageLoad = async ({ fetch }) => ({
	info: await api<LegalInfo>('/legal', {}, fetch).catch(() => null)
});
