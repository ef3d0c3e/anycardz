import { describe, expect, it } from 'vitest';
import { deviceName } from './device.ts';

describe('deviceName', () => {
	it('names common browsers and systems', () => {
		expect(
			deviceName('Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0')
		).toBe('Firefox · Linux');
		expect(
			deviceName(
				'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Mobile Safari/537.36'
			)
		).toBe('Chrome · Android');
		expect(
			deviceName(
				'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'
			)
		).toBe('Safari · iPhone');
		expect(
			deviceName(
				'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0'
			)
		).toBe('Edge · Windows');
	});
	it('gives up on the unknown', () => {
		expect(deviceName('')).toBeNull();
		expect(deviceName('curl/8.0')).toBeNull();
	});
});
