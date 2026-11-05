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
	/** The free pack stand-in (slug FREE_PACK), not a real collection */
	free?: boolean;
	/** Coins for owning every card of the set, and every one as a foil */
	set_reward: number;
	foil_set_reward: number;
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
	free_packs: FreePacksDto;
	/** Packs to open before a legendary is guaranteed, the next one included */
	pity_left: number;
	missions: MissionsDto;
}

export interface MissionDto {
	key: string;
	goal: number;
	progress: number;
	coins: number;
	done: boolean;
}

export interface MissionsDto {
	items: MissionDto[];
	/** All three done: the free pack was given */
	bonus: boolean;
	resets_at: string;
}

export interface FreePacksDto {
	ready: boolean;
	/** When they are claimable, if not now */
	next_at: string | null;
	count: number;
	every_hours: number;
}

/** The pile entry of a free pack: it draws from every collection */
export const FREE_PACK = '*';

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
	/** Earned keys: set:<slug>, foilset:<slug>, achievements */
	achievements: string[];
}

/** A set completed or an achievement earned */
export interface Unlock {
	key: string;
	coins: number;
}

export interface EarnedDto extends Unlock {
	at: string;
}

export interface FeatEvent {
	user: string;
	key: string;
	at: string;
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
	/** Packs to the guaranteed legendary after this one */
	pity_left: number;
	unlocked: Unlock[];
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
	/** Completed sets and achievements, newest first */
	achievements: EarnedDto[];
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
	| 'trade_cancelled'
	| 'achievement'
	| 'market_sold'
	| 'mission';

export interface NotificationDto {
	id: number;
	kind: NotificationKind;
	actor: string | null;
	actor_avatar: string | null;
	trade: number | null;
	/** The achievement key for "achievement"; for "market_sold", JSON: card, foil, price, net */
	detail: string | null;
	read: boolean;
	at: string;
}

export type Metric =
	'packs' | 'trades' | 'sets' | 'friends' | 'cards' | 'streak' | 'roulette_streak';

export interface AchievementDto {
	id: string;
	coins: number;
	metric: Metric | null;
	goal: number | null;
	/** The player's count now, for goals */
	have: number | null;
	earned_at: string | null;
}

export interface AchievementsDto {
	items: AchievementDto[];
	sets: number;
	foil_sets: number;
}

export const BOARDS = ['sets', 'achievements', 'legendaries', 'foils', 'trades', 'streak'] as const;
export type Board = (typeof BOARDS)[number];
export type Period = 'week' | 'all';

export interface RankDto {
	rank: number;
	username: string;
	avatar: string | null;
	value: number;
	me: boolean;
}

export interface LeaderboardDto {
	board: Board;
	period: Period;
	top: RankDto[];
	/** The caller's row, null when not ranked */
	me: RankDto | null;
}

export interface ListingDto {
	id: number;
	card: CardDto;
	/** The foil pattern, for a foil copy */
	foil: string | null;
	price: number;
	seller: string;
	seller_avatar: string | null;
	created_at: string;
	mine: boolean;
}

export interface MarketPage {
	items: ListingDto[];
	total: number;
	page: number;
	per_page: number;
}

export interface SaleDto {
	id: number;
	card: CardDto;
	foil: string | null;
	price: number;
	fee: number;
	buyer: string | null;
	at: string | null;
}

export interface MarketMine {
	open: ListingDto[];
	sold: SaleDto[];
	max_listings: number;
	fee_percent: number;
}

export interface MarketSoldDetail {
	card: string;
	foil: boolean;
	price: number;
	net: number;
}
