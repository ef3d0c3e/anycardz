// Offer lines of a trade: editing them and spotting what the partner changed.

export interface Line {
	card: string;
	foil: boolean;
	count: number;
}

const key = (l: Pick<Line, 'card' | 'foil'>) => `${l.card}|${l.foil}`;

/** Adds `delta` copies of a card to a side; a line reaching zero goes away */
export function change(lines: Line[], card: string, foil: boolean, delta: number): Line[] {
	const out = lines.map((l) => ({ ...l }));
	const at = out.findIndex((l) => l.card === card && l.foil === foil);
	if (at < 0) {
		if (delta > 0) out.push({ card, foil, count: delta });
		return out;
	}
	out[at].count += delta;
	return out.filter((l) => l.count > 0);
}

/** Copies of a card already on a side */
export const onSide = (lines: Line[], card: string, foil: boolean) =>
	lines.find((l) => l.card === card && l.foil === foil)?.count ?? 0;

export interface Diff {
	/** Copies that appeared, per line */
	added: Line[];
	/** Copies that went away, per line */
	removed: Line[];
}

/** What changed from `before` to `after` on one side */
export function diff(before: Line[], after: Line[]): Diff {
	const was = new Map(before.map((l) => [key(l), l.count]));
	const now = new Map(after.map((l) => [key(l), l.count]));
	const added: Line[] = [];
	const removed: Line[] = [];
	for (const l of after) {
		const d = l.count - (was.get(key(l)) ?? 0);
		if (d > 0) added.push({ ...l, count: d });
	}
	for (const l of before) {
		const d = l.count - (now.get(key(l)) ?? 0);
		if (d > 0) removed.push({ ...l, count: d });
	}
	return { added, removed };
}

export const isEmpty = (d: Diff) => !d.added.length && !d.removed.length;
