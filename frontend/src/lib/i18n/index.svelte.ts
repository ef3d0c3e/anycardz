import { getContext, setContext } from 'svelte';
import en from './en';
import fr from './fr';

export type Lang = 'en' | 'fr';
export const LANGS: Lang[] = ['en', 'fr'];
type Base<K> = K extends `${infer B}_one` | `${infer B}_other` ? B : K;
/** Every key, plurals named without their _one/_other suffix */
export type Key = keyof typeof en;
export type TKey = Base<Key> | Key;

const dicts: Record<Lang, Partial<Record<Key, string>>> = { en, fr };

export function parseLang(value: string | null | undefined): Lang | undefined {
	const code = value?.slice(0, 2).toLowerCase();
	return LANGS.find((l) => l === code);
}

const plural = (lang: Lang, n: number) => ((lang === 'fr' ? n < 2 : n === 1) ? 'one' : 'other');

export class I18n {
	lang = $state<Lang>('en');

	constructor(lang: Lang) {
		this.lang = lang;
	}

	/** `t('lobby.packs_waiting', { n: 3 })` picks the _one / _other form from n */
	t = (key: TKey, params: Record<string, string | number> = {}): string => {
		const dict = dicts[this.lang] as Record<string, string | undefined>;
		const base = en as Record<string, string>;
		let k: string = key;
		if (typeof params.n === 'number') {
			const pk = `${key}_${plural(this.lang, params.n)}`;
			if (pk in base) k = pk;
		}
		const text = dict[k] ?? base[k] ?? key;
		return text.replace(/\{(\w+)\}/g, (_, p) => {
			const v = params[p];
			return typeof v === 'number' ? this.num(v) : (v ?? '');
		});
	};

	num = (n: number) => n.toLocaleString(this.lang === 'fr' ? 'fr-FR' : 'en-US');

	/** Message for an API error code */
	error = (e: unknown) => {
		const code = (e as { code?: string })?.code ?? 'internal';
		const wait = (e as { retryAfter?: number })?.retryAfter;
		if (code === 'too_many_attempts' && wait) {
			return wait < 90
				? this.t('errors.too_many_attempts', { n: wait })
				: this.t('errors.too_many_attempts_min', { n: Math.ceil(wait / 60) });
		}
		const key = `errors.${code}` as Key;
		return key in en ? this.t(key) : this.t('errors.internal');
	};
}

const KEY = Symbol('i18n');
// Context, not a module variable: on the server one module serves every visitor
export const setI18n = (lang: Lang) => setContext(KEY, new I18n(lang));
export const getI18n = () => getContext<I18n>(KEY);
