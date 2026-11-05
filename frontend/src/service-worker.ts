/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

// Push only: no caching, the site is always fetched fresh
const sw = self as unknown as ServiceWorkerGlobalScope;

interface Message {
	title: string;
	body: string;
	url: string;
	tag: string;
}

sw.addEventListener('install', () => sw.skipWaiting());
sw.addEventListener('activate', (e) => e.waitUntil(sw.clients.claim()));

sw.addEventListener('push', (e) => {
	let m: Message;
	try {
		m = e.data?.json();
	} catch {
		return;
	}
	if (!m?.title) return;
	e.waitUntil(
		sw.registration.showNotification(m.title, {
			body: m.body,
			tag: m.tag,
			icon: '/icons/icon-192.png',
			badge: '/icons/icon-192.png',
			data: { url: m.url }
		})
	);
});

// A tap opens the page it is about, in a tab of the site when one is open
sw.addEventListener('notificationclick', (e) => {
	e.notification.close();
	const url = new URL(e.notification.data?.url ?? '/', sw.location.origin).href;
	e.waitUntil(
		(async () => {
			const tabs = await sw.clients.matchAll({ type: 'window', includeUncontrolled: true });
			const tab = tabs.find((c) => new URL(c.url).origin === sw.location.origin);
			if (tab) {
				await tab.focus();
				await tab.navigate(url).catch(() => {});
			} else {
				await sw.clients.openWindow(url);
			}
		})()
	);
});
