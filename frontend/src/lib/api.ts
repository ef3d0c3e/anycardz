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

export const artUrl = (slug: string, width: 420 | 840 = 420) => `/api/cards/${slug}/art?w=${width}`;
export const assetUrl = (name: string) => `/api/assets/${name}`;
