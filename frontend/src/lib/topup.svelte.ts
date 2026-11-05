import { getContext, setContext } from 'svelte';

interface Request {
	/** Coins missing for what the player tried to do */
	need?: number;
	/** What the coins are for, shown in the sheet ("a Trains booster") */
	reason?: string;
	/** Runs once the coins are in: finishes the purchase that was short */
	then?: () => Promise<void> | void;
}

/** Opens the top-up sheet from anywhere in the app */
export class TopupState {
	open = $state(false);
	request = $state<Request>({});
	ask = (request: Request = {}) => {
		this.request = request;
		this.open = true;
	};
	close = () => {
		this.open = false;
	};
}

const KEY = Symbol('topup');
export const setTopup = () => setContext(KEY, new TopupState());
export const getTopup = () => getContext<TopupState>(KEY);
