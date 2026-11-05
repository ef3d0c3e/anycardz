/** Error codes are stable strings from the API, see backend/src/error.rs */
export class ApiError extends Error {
	constructor(
		public status: number,
		public code: string,
		/** Seconds to wait, on too_many_attempts */
		public retryAfter?: number
	) {
		super(code);
	}
}

type Fetch = typeof fetch;

/** Calls the API. Pass SvelteKit's `fetch` from load functions so SSR forwards cookies. */
export async function api<T>(
	path: string,
	init: { method?: string; json?: unknown } = {},
	f: Fetch = fetch
): Promise<T> {
	let res: Response;
	try {
		res = await f(`/api${path}`, {
			method: init.method ?? (init.json === undefined ? 'GET' : 'POST'),
			headers: init.json === undefined ? undefined : { 'content-type': 'application/json' },
			body: init.json === undefined ? undefined : JSON.stringify(init.json)
		});
	} catch {
		throw new ApiError(0, 'network');
	}
	const body = await res.json().catch(() => null);
	if (!res.ok) throw new ApiError(res.status, body?.error ?? 'internal', body?.retry_after);
	return body as T;
}

export const post = <T>(path: string, json: unknown = {}, f?: Fetch) => api<T>(path, { json }, f);

export type ArtWidth = 96 | 240 | 420 | 840;
export const artUrl = (slug: string, width: ArtWidth = 420) => `/api/cards/${slug}/art?w=${width}`;
/**
 * Card art at the sizes the API renders, for the browser to pick by displayed
 * size. Cards in grids stop at 420 (sharp enough even on 3x screens at their
 * size); 840 is for a card shown large.
 */
export const artSrcset = (slug: string, large = false) =>
	(large ? ([420, 840] as const) : ([240, 420] as const))
		.map((w) => `${artUrl(slug, w)} ${w}w`)
		.join(', ');
export const assetUrl = (name: string) => `/api/assets/${name}`;
