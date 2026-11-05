import type { Handle, HandleFetch } from '@sveltejs/kit/hooks';
import { parseLang } from '#lib/i18n/index.svelte.ts';
import { parseTheme } from '#lib/theme.svelte.ts';

/** Where the server reaches the API. The browser goes through /api instead. */
const API_URL = process.env.API_URL ?? 'http://127.0.0.1:8080';

export const handle: Handle = async ({ event, resolve }) => {
	event.locals.theme = parseTheme(event.cookies.get('theme'));
	event.locals.lang =
		parseLang(event.cookies.get('lang')) ??
		parseLang(event.request.headers.get('accept-language')?.split(',')[0]) ??
		'en';
	// The theme is in the HTML from the first byte: no flash of the wrong one
	return resolve(event, {
		transformPageChunk: ({ html }) =>
			html.replace('%theme%', event.locals.theme).replace('%lang%', event.locals.lang)
	});
};

/** Server-side fetches of /api go straight to the API, with the visitor's cookies */
export const handleFetch: HandleFetch = async ({ event, request, fetch }) => {
	const url = new URL(request.url);
	if (url.origin === event.url.origin && url.pathname.startsWith('/api/')) {
		const target = new URL(url.pathname + url.search, API_URL);
		const headers = new Headers(request.headers);
		const cookie = event.request.headers.get('cookie');
		if (cookie) headers.set('cookie', cookie);
		// A language picked a moment ago may not be in the request's cookie header yet
		headers.set('accept-language', event.locals.lang);
		request = new Request(target, {
			method: request.method,
			headers,
			body: request.body,
			// @ts-expect-error duplex is required by Node for streamed bodies
			duplex: 'half'
		});
	}
	return fetch(request);
};
