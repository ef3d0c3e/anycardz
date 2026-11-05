import type { OwnedDto } from './types.ts';

/**
 * The foil pattern of a player's i-th foil copy (newest first). The fallback
 * only covers data without patterns: every foil copy has one.
 */
export function foilOf(
	o: Pick<OwnedDto, 'foils' | 'patterns'> | null | undefined,
	i = 0
): string | null {
	if (!o || o.foils <= i) return null;
	return o.patterns?.[i] ?? 'foil3';
}
