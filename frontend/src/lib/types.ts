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
	avatar: string | null;
	/** Friend requests waiting for an answer */
	friend_requests: number;
	/** Unread notifications */
	notifications: number;
	email: string;
	coins: number;
	pile: string[];
	streak: StreakDto;
	/** The current terms are not accepted yet: asked before playing on */
	terms_outdated: boolean;
}

export interface OwnedDto {
	card: string;
	count: number;
	foils: number;
	/** Pattern of each foil copy, newest first */
	patterns: string[];
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

export interface PullEvent {
	user: string;
	card: string;
	rarity: Rarity;
	foil: string | null;
	source: 'pack' | 'tradeup' | 'roulette';
	at: string;
}

export interface ChatEvent {
	id: number;
	user: string;
	body: string;
	card: string | null;
	foil: string | null;
	at: string;
}

export type Relation =
	'self' | 'none' | 'friends' | 'request_sent' | 'request_received' | 'blocked';

export interface ProfileDto {
	username: string;
	bio: string;
	avatar: string | null;
	joined: string;
	online: boolean;
	cards: number;
	copies: number;
	foils: number;
	collections: { slug: string; owned: number; total: number }[];
	relation: Relation;
}

export interface FriendDto {
	username: string;
	avatar: string | null;
	online: boolean;
}

export interface FriendsDto {
	friends: FriendDto[];
	incoming: FriendDto[];
	outgoing: FriendDto[];
}

export type NotificationKind =
	| 'friend_request'
	| 'friend_accepted'
	| 'trade_opened'
	| 'trade_changed'
	| 'trade_sealed'
	| 'trade_done'
	| 'trade_cancelled';

export interface NotificationDto {
	id: number;
	kind: NotificationKind;
	actor: string | null;
	actor_avatar: string | null;
	trade: number | null;
	read: boolean;
	at: string;
}
