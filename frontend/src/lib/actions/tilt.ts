import type { Action } from 'svelte/action';

const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Tilts the element toward the pointer and feeds --mx/--my to the foil and glare */
export const tilt: Action<HTMLElement, boolean | undefined> = (node, enabled = true) => {
	let on = enabled;
	const move = (e: PointerEvent) => {
		if (!on || reduced()) return;
		const r = node.getBoundingClientRect();
		const x = (e.clientX - r.left) / r.width;
		const y = (e.clientY - r.top) / r.height;
		node.classList.add('tilting');
		node.style.setProperty('--rx', `${((0.5 - y) * 18).toFixed(2)}deg`);
		node.style.setProperty('--ry', `${((x - 0.5) * 22).toFixed(2)}deg`);
		node.style.setProperty('--mx', (x * 100).toFixed(1));
		node.style.setProperty('--my', (y * 100).toFixed(1));
	};
	const leave = () => {
		node.classList.remove('tilting');
		node.style.removeProperty('--rx');
		node.style.removeProperty('--ry');
	};
	node.addEventListener('pointermove', move);
	node.addEventListener('pointerleave', leave);
	return {
		update(v) {
			on = v ?? true;
			if (!on) leave();
		},
		destroy() {
			node.removeEventListener('pointermove', move);
			node.removeEventListener('pointerleave', leave);
		}
	};
};
