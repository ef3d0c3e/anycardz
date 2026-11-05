import { describe, expect, it } from 'vitest';
import { keyBytes } from './push.ts';

describe('push keys', () => {
	it('decodes base64url without padding', () => {
		expect([...keyBytes('AQID_-8')]).toEqual([1, 2, 3, 255, 239]);
		expect(keyBytes('A'.repeat(87)).length).toBe(65);
	});
});
