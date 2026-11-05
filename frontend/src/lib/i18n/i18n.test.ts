import { describe, expect, it } from 'vitest';
import { I18n } from './index.svelte.ts';
import { ApiError } from '#lib/api.ts';

describe('error messages', () => {
	it('say how long to wait after too many attempts', () => {
		const en = new I18n('en');
		expect(en.error(new ApiError(429, 'too_many_attempts', 42))).toBe(
			'Too many attempts. Try again in 42 seconds.'
		);
		expect(en.error(new ApiError(429, 'too_many_attempts', 1))).toBe(
			'Too many attempts. Try again in 1 second.'
		);
		expect(en.error(new ApiError(429, 'too_many_attempts', 540))).toBe(
			'Too many attempts. Try again in 9 minutes.'
		);
		const fr = new I18n('fr');
		expect(fr.error(new ApiError(429, 'too_many_attempts', 30))).toBe(
			'Trop de tentatives. Réessaie dans 30 secondes.'
		);
	});
	it('fall back to a generic message for unknown codes', () => {
		expect(new I18n('en').error(new ApiError(500, 'what_is_this'))).toBe(
			'Something broke on our side. Try again in a moment.'
		);
	});
});
