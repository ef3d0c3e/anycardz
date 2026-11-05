import { post } from './api.ts';
import type { I18n } from './i18n/index.svelte.ts';
import type { Live } from './live.svelte.ts';
import type { Toasts } from './session.svelte.ts';
import type { ChatEvent } from './types.ts';

/** Posts a card the player owns to the shoutbox, and shows the panel where it is hidden */
export async function brag(
	card: string,
	foil: boolean,
	deps: { i18n: I18n; live: Live; toasts: Toasts }
) {
	const { i18n, live, toasts } = deps;
	try {
		live.sent(
			await post<ChatEvent>('/shoutbox', { body: i18n.t('live.brag_text'), card: { card, foil } })
		);
		toasts.show(i18n.t('live.bragged'));
		if (!matchMedia('(min-width: 1080px)').matches) live.panelOpen = true;
	} catch (e) {
		toasts.show(i18n.error(e), 'warn');
	}
}
