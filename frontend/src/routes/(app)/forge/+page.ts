import { api } from '#lib/api.ts';
import type { BinderDto } from '#lib/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => ({
	binder: await api<BinderDto>('/binder?details=1', {}, fetch)
});
