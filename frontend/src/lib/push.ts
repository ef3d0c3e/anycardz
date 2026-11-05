import { api } from './api.ts';

export type PushState = 'unsupported' | 'off-server' | 'blocked' | 'on' | 'off';

/** The VAPID key as the Push API wants it */
export function keyBytes(b64url: string): Uint8Array<ArrayBuffer> {
	const b64 = (b64url + '='.repeat((4 - (b64url.length % 4)) % 4))
		.replace(/-/g, '+')
		.replace(/_/g, '/');
	const raw = atob(b64);
	const out = new Uint8Array(new ArrayBuffer(raw.length));
	for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
	return out;
}

/** The service worker that shows pushes: SvelteKit registers it, else it is registered here */
async function worker(): Promise<ServiceWorkerRegistration> {
	if (!(await navigator.serviceWorker.getRegistration())) {
		await navigator.serviceWorker.register('/service-worker.js', {
			type: import.meta.env.DEV ? 'module' : 'classic'
		});
	}
	return navigator.serviceWorker.ready;
}

export const supported = () =>
	typeof window !== 'undefined' &&
	'serviceWorker' in navigator &&
	'PushManager' in window &&
	'Notification' in window;

/** Where push stands on this device */
export async function pushState(): Promise<{ state: PushState; key: string | null }> {
	if (!supported()) return { state: 'unsupported', key: null };
	const s = await api<{ key: string | null; subscribed: boolean }>('/push');
	if (!s.key) return { state: 'off-server', key: null };
	if (Notification.permission === 'denied') return { state: 'blocked', key: s.key };
	// The server forgets a device on sign-out; the browser may still hold one
	const sub = await (await worker()).pushManager.getSubscription();
	return { state: s.subscribed && sub ? 'on' : 'off', key: s.key };
}

/** Asks for permission, subscribes the browser and tells the server */
export async function turnOn(key: string): Promise<PushState> {
	const permission = await Notification.requestPermission();
	if (permission !== 'granted') return permission === 'denied' ? 'blocked' : 'off';
	const reg = await worker();
	let sub = await reg.pushManager.getSubscription();
	// A subscription made with another server key cannot be reused
	if (sub && sub.options.applicationServerKey) {
		const old = new Uint8Array(sub.options.applicationServerKey);
		const now = keyBytes(key);
		if (old.length !== now.length || old.some((b, i) => b !== now[i])) {
			await sub.unsubscribe();
			sub = null;
		}
	}
	sub ??= await reg.pushManager.subscribe({
		userVisibleOnly: true,
		applicationServerKey: keyBytes(key)
	});
	await api('/push', { method: 'PUT', json: sub.toJSON() });
	return 'on';
}

export async function turnOff(): Promise<PushState> {
	const sub = await (await worker()).pushManager.getSubscription();
	await sub?.unsubscribe().catch(() => false);
	await api('/push', { method: 'DELETE' });
	return 'off';
}
