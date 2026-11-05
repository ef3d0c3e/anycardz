import { describe, expect, it } from 'vitest';
import { ago } from './live.svelte.ts';

describe('ago', () => {
	const now = Date.parse('2026-11-05T12:00:00Z');
	const at = (s: number) => new Date(now - s * 1000).toISOString();
	it('rounds to the right unit', () => {
		expect(ago(at(10), 'en', now)).toBe('now');
		expect(ago(at(180), 'en', now)).toBe('3 minutes ago');
		expect(ago(at(7200), 'en', now)).toBe('2 hours ago');
		expect(ago(at(86_400 * 2), 'en', now)).toBe('2 days ago');
	});
	it('speaks the page language', () => {
		expect(ago(at(180), 'fr', now)).toBe('il y a 3 minutes');
	});
});
