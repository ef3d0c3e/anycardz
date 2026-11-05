// See https://svelte.dev/docs/kit/types#app.d.ts
import type { Lang } from '#lib/i18n/index.svelte.ts';
import type { Theme } from '#lib/theme.svelte.ts';

declare global {
	namespace App {
		interface Locals {
			theme: Theme;
			lang: Lang;
		}
	}
}

export {};
