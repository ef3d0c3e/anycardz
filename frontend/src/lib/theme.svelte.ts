import { getContext, setContext } from 'svelte';

export type Theme = 'system' | 'light' | 'dark';
export const THEMES: Theme[] = ['system', 'light', 'dark'];
export const parseTheme = (v: string | undefined): Theme => THEMES.find((t) => t === v) ?? 'system';

const YEAR = 60 * 60 * 24 * 365;
export function setCookie(name: string, value: string) {
	document.cookie = `${name}=${encodeURIComponent(value)}; Path=/; Max-Age=${YEAR}; SameSite=Lax`;
}

export class ThemeState {
	current = $state<Theme>('system');
	constructor(t: Theme) {
		this.current = t;
	}
	set(t: Theme) {
		this.current = t;
		setCookie('theme', t);
		document.documentElement.dataset.theme = t;
	}
}

const KEY = Symbol('theme');
export const setTheme = (t: Theme) => setContext(KEY, new ThemeState(t));
export const getTheme = () => getContext<ThemeState>(KEY);
