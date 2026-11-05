// Sequencing of a pack's reels, apart from the DOM so it can be unit tested.
import type { Rarity } from '#lib/types.ts';

export type Mode = 'one' | 'all';

export interface ReelCard {
	rarity: Rarity;
	foil: string | null;
}

/** How long a reel turns before it lands on its own, in ms */
export function spinDuration(mode: Mode, index: number, rarity: Rarity): number {
	if (mode === 'all') return 2300 + index * 480 + (rarity === 'legendary' ? 900 : 0);
	return { common: 1900, rare: 2700, legendary: 3900 }[rarity];
}

/** One by one: how long a landed card stays before the next reel, unless tapped */
export const landPause = (c: ReelCard) => (c.rarity === 'common' && !c.foil ? 1500 : 3200);

/**
 * The taps and skips of one pack. Every pack gets a new run, so a tap on one
 * pack can never stop the reels of the next (AC-43).
 */
export class ReelRun {
	#stopCurrent = false;
	#skipAll = false;
	#wake: (() => void) | null = null;

	/** The turning reels should land now */
	get stopping() {
		return this.#stopCurrent || this.#skipAll;
	}
	get skippingAll() {
		return this.#skipAll;
	}
	/** A tap: moves on from a landed card, else stops the turning reels */
	tap() {
		if (this.#wake) this.#wake();
		else this.#stopCurrent = true;
	}
	/** Skip: lands everything left in this pack */
	skip() {
		this.#skipAll = true;
		this.#stopCurrent = true;
		this.#wake?.();
	}
	/** One by one: a tap stops one reel, not the ones after it */
	nextReel() {
		this.#stopCurrent = false;
	}
	/** Waits `ms`, or less if tapped or skipped */
	pause(ms: number): Promise<void> {
		if (this.#skipAll) return Promise.resolve();
		return new Promise((resolve) => {
			const done = () => {
				clearTimeout(timer);
				this.#wake = null;
				resolve();
			};
			const timer = setTimeout(done, ms);
			this.#wake = done;
		});
	}
}

/** What the page does for the sequence: render, animate, react */
export interface ReelStage {
	/** Renders reels for these cards (indexes into the pack) and waits for the DOM */
	show(indexes: number[]): Promise<void>;
	/** Turns the reel in `slot` for `ms`; must land early once `run.stopping` */
	spin(slot: number, ms: number, run: ReelRun): Promise<void>;
	/** The reel in `slot` landed on card `index` */
	land(slot: number, index: number): void;
	/** Card `index` goes to the tray */
	tray(index: number): void;
	/** Where the sequence is, for hints */
	status(phase: 'spinning' | 'landed', index: number): void;
}

/** Plays a whole pack: five reels at once, or one after the other */
export async function playPack(cards: ReelCard[], mode: Mode, run: ReelRun, stage: ReelStage) {
	if (mode === 'all') {
		const all = cards.map((_, i) => i);
		await stage.show(all);
		stage.status('spinning', 0);
		await Promise.all(
			cards.map((c, i) =>
				stage.spin(i, spinDuration('all', i, c.rarity), run).then(() => {
					stage.land(i, i);
					stage.tray(i);
				})
			)
		);
		return;
	}
	for (let i = 0; i < cards.length; i++) {
		run.nextReel();
		await stage.show([i]);
		stage.status('spinning', i);
		await stage.spin(0, spinDuration('one', i, cards[i].rarity), run);
		stage.land(0, i);
		stage.status('landed', i);
		await run.pause(landPause(cards[i]));
		stage.tray(i);
	}
}
