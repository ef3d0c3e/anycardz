/** "Firefox on Linux" from a user agent; null when nothing is recognized */
export function deviceName(ua: string): string | null {
	const browser = /Edg\//.test(ua)
		? 'Edge'
		: /OPR\//.test(ua)
			? 'Opera'
			: /Firefox\//.test(ua)
				? 'Firefox'
				: /Chrome\/|CriOS\//.test(ua)
					? 'Chrome'
					: /Safari\//.test(ua)
						? 'Safari'
						: null;
	const os = /iPhone/.test(ua)
		? 'iPhone'
		: /iPad/.test(ua)
			? 'iPad'
			: /Android/.test(ua)
				? 'Android'
				: /Windows/.test(ua)
					? 'Windows'
					: /Mac OS X|Macintosh/.test(ua)
						? 'macOS'
						: /CrOS/.test(ua)
							? 'ChromeOS'
							: /Linux/.test(ua)
								? 'Linux'
								: null;
	if (browser && os) return `${browser} · ${os}`;
	return browser ?? os;
}
