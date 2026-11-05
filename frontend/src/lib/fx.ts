// Browser-only effects: sounds, flying coins and packs, confetti bursts.

export const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
export const sleep = (ms: number) =>
	new Promise((r) => setTimeout(r, reducedMotion() ? Math.min(ms, 50) : ms));

const SOUND_KEY = 'anycardz-sound';
export function soundOn(): boolean {
	try {
		return localStorage.getItem(SOUND_KEY) !== 'off';
	} catch {
		return true;
	}
}
export function setSound(on: boolean) {
	try {
		localStorage.setItem(SOUND_KEY, on ? 'on' : 'off');
	} catch {
		/* remembered for this visit only */
	}
}

let ctx: AudioContext | null = null;
export function tone(f = 600, d = 0.06, type: OscillatorType = 'square', v = 0.035, when = 0) {
	if (!soundOn()) return;
	try {
		ctx ??= new AudioContext();
		const t = ctx.currentTime + when;
		const o = ctx.createOscillator();
		const g = ctx.createGain();
		o.type = type;
		o.frequency.setValueAtTime(f, t);
		g.gain.setValueAtTime(v, t);
		g.gain.exponentialRampToValueAtTime(0.0001, t + d);
		o.connect(g).connect(ctx.destination);
		o.start(t);
		o.stop(t + d + 0.03);
	} catch {
		/* sound is optional */
	}
}
export const arp = (notes: number[], step = 0.07, type: OscillatorType = 'triangle', v = 0.05) =>
	notes.forEach((f, i) => tone(f, 0.18, type, v, i * step));

/** Coins flying from an element to the wallet in the top bar */
export function flyCoins(from: Element, n = 10): Promise<void> {
	const wallet = document.getElementById('wallet');
	if (!wallet || reducedMotion()) return Promise.resolve();
	const a = from.getBoundingClientRect();
	const b = wallet.getBoundingClientRect();
	const flights = Array.from({ length: n }, (_, i) => {
		const c = document.createElement('span');
		c.className = 'flycoin';
		document.body.appendChild(c);
		const x0 = a.left + a.width / 2 + (Math.random() - 0.5) * a.width * 0.6;
		const y0 = a.top + a.height / 2 + (Math.random() - 0.5) * a.height * 0.4;
		const x1 = b.left + 18;
		const y1 = b.top + b.height / 2;
		const mx = (x0 + x1) / 2 + (Math.random() - 0.5) * 140;
		const my = Math.min(y0, y1) - 40 - Math.random() * 90;
		return c
			.animate(
				[
					{ transform: `translate(${x0}px,${y0}px) scale(.4)`, opacity: 0 },
					{ transform: `translate(${x0}px,${y0 - 24}px) scale(1)`, opacity: 1, offset: 0.15 },
					{ transform: `translate(${mx}px,${my}px) scale(1)`, offset: 0.55 },
					{ transform: `translate(${x1}px,${y1}px) scale(.5)`, opacity: 0.9 }
				],
				{ duration: 720, delay: i * 50, easing: 'cubic-bezier(.45,0,.6,1)', fill: 'forwards' }
			)
			.finished.then(() => {
				c.remove();
				tone(1500 + i * 45, 0.03, 'triangle', 0.03);
			});
	});
	return Promise.all(flights).then(() => {
		wallet.classList.remove('ping');
		void wallet.offsetWidth;
		wallet.classList.add('ping');
	});
}

/** A copy of `el` flying into `target` (a pack into the Open tab) */
export function flyTo(el: Element, target: Element): Promise<void> {
	if (reducedMotion()) return Promise.resolve();
	const a = el.getBoundingClientRect();
	const t = target.getBoundingClientRect();
	const ghost = el.cloneNode(true) as HTMLElement;
	Object.assign(ghost.style, {
		position: 'fixed',
		left: '0',
		top: '0',
		width: `${a.width}px`,
		zIndex: '9999',
		pointerEvents: 'none',
		margin: '0'
	});
	document.body.appendChild(ghost);
	const dx = t.left + t.width / 2 - a.width / 2;
	const dy = t.top + t.height / 2 - a.height / 2;
	return ghost
		.animate(
			[
				{ transform: `translate(${a.left}px,${a.top}px) rotate(0) scale(1)` },
				{
					transform: `translate(${(a.left + dx) / 2}px,${Math.min(a.top, dy) - 80}px) rotate(-20deg) scale(.7)`,
					offset: 0.5
				},
				{ transform: `translate(${dx}px,${dy}px) rotate(10deg) scale(.15)`, opacity: 0.6 }
			],
			{ duration: 750, easing: 'cubic-bezier(.4,0,.6,1)', fill: 'forwards' }
		)
		.finished.then(() => ghost.remove());
}

/** Confetti from the middle of `host` */
export function burst(host: HTMLElement, color = '#ffcf5c', n = 26) {
	if (reducedMotion()) return;
	const b = document.createElement('div');
	b.className = 'burst';
	b.style.setProperty('--bc', color);
	host.appendChild(b);
	for (let i = 0; i < n; i++) {
		const p = document.createElement('i');
		b.appendChild(p);
		const ang = Math.random() * Math.PI * 2;
		const d = 80 + Math.random() * 140;
		p.animate(
			[
				{ transform: 'translate(0,0) rotate(0)', opacity: 1 },
				{
					transform: `translate(${Math.cos(ang) * d}px,${Math.sin(ang) * d}px) rotate(${Math.random() * 720}deg)`,
					opacity: 0
				}
			],
			{ duration: 700 + Math.random() * 500, easing: 'cubic-bezier(.1,.8,.3,1)', fill: 'forwards' }
		);
	}
	setTimeout(() => b.remove(), 1400);
}

export const pad3 = (n: number) => String(n).padStart(3, '0');
