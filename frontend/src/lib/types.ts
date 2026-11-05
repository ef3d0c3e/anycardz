export type Rarity = 'common' | 'rare' | 'legendary';

export interface CardDto {
	slug: string;
	number: number;
	rarity: Rarity;
	title: string;
	description: string;
	primary: string;
	secondary: string;
	collection: string;
	total: number;
}

export interface CollectionDto {
	slug: string;
	number: number;
	title: string;
	description: string;
	primary: string;
	secondary: string;
	total: number;
	price: number;
	/** Cards per rarity in the collection */
	rarities?: Partial<Record<Rarity, number>>;
	featured: CardDto;
	cards?: CardDto[];
}

export interface StreakDto {
	day: number;
	best: number;
	claimed_today: boolean;
	reward: number;
	next_reward: number;
	rewards: number[];
}

export interface MeDto {
	id: number;
	username: string;
	email: string;
	coins: number;
	pile: string[];
	streak: StreakDto;
}

export interface OwnedDto {
	card: string;
	count: number;
	foils: number;
}

export interface BinderDto {
	cards: OwnedDto[];
	collections: Record<string, number>;
	/** With ?details=1 */
	details?: Record<string, CardDto> | null;
}

export interface PulledDto {
	card: string;
	rarity: Rarity;
	foil: string | null;
	is_new: boolean;
	copies: number;
}

export interface OpenedDto {
	collection: string;
	cards: PulledDto[];
	pile: string[];
}

export interface TradeLine {
	card: string;
	foil: boolean;
	count: number;
}

export interface TradeSide {
	coins: number;
	cards: TradeLine[];
	sealed: boolean;
}

export interface TradeDto {
	id: number;
	version: number;
	state: 'open' | 'done' | 'cancelled';
	partner: string;
	/** What I give */
	mine: TradeSide;
	/** What the partner gives */
	theirs: TradeSide;
	edited_by: 'me' | 'partner' | null;
	updated_at: string;
	details: Record<string, CardDto>;
}

export interface TradeSummary {
	id: number;
	partner: string;
	state: TradeDto['state'];
	mine: number;
	theirs: number;
	you_sealed: boolean;
	they_sealed: boolean;
	edited_by: 'me' | 'partner' | null;
	updated_at: string;
}

export interface PlayerCardsDto {
	cards: OwnedDto[];
	details: Record<string, CardDto>;
}
