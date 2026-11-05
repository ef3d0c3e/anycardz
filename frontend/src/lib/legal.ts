// Terms of service and privacy notice, in both languages. They describe what
// the code does: change them together with it.
//
// TERMS_VERSION must match TERMS_VERSION in backend/src/routes/legal.rs. Raise
// both when a change needs players to agree again (a new rule, new data kept);
// fixing a typo does not.
export const TERMS_VERSION = 1;

export interface LegalInfo {
	version: number;
	updated: string;
	operator: string | null;
	contact: string | null;
}

export interface Section {
	title: string;
	/** Paragraphs; `{operator}` and `{contact}` are filled in */
	body: string[];
}

type Doc = { title: string; intro: string; sections: Section[] };

const en: { terms: Doc; privacy: Doc; operator: string; contact: string } = {
	operator: 'the operator of this site',
	contact: 'the site’s contact address',
	terms: {
		title: 'Terms of service',
		intro:
			'These terms apply to everyone who plays AnyCardz. By creating an account you agree to them. AnyCardz is run by {operator}.',
		sections: [
			{
				title: 'The game',
				body: [
					'AnyCardz is a card collecting game: you get coins, buy boosters, open them, and collect, forge and trade the cards inside.',
					'The game may change at any time: prices, odds, rewards, collections and features can be adjusted, added or removed.'
				]
			},
			{
				title: 'Coins and cards have no money value',
				body: [
					'Coins, boosters and cards are game items. They are not money, cannot be exchanged for money, and are not your property: you get a right to use them in the game, nothing more.',
					'Coin packs are free for now: no payment is taken. If paid packs ever come, they will be announced and these terms will change first.',
					'Selling, buying or trading accounts, coins or cards for real money or anything outside the game is not allowed.'
				]
			},
			{
				title: 'Your account',
				body: [
					'One person, one account. Keep your password to yourself: what is done with your account is your responsibility.',
					'Your username must not impersonate someone, insult, or be obscene. We may rename or close accounts that break these terms.',
					'You can delete your account at any time from Settings. Deletion is immediate and final.'
				]
			},
			{
				title: 'Playing fair',
				body: [
					'Do not use bots, scripts or bugs to gain coins or cards, to slow the site down, or to get at other players’ data. If you find a bug, tell us at {contact}.',
					'Using several accounts to move coins or cards to one of them is not allowed.',
					'We may remove items gained by breaking these rules, or suspend the accounts involved.'
				]
			},
			{
				title: 'Trades',
				body: [
					'Trades happen between players. Check every offer before sealing it: a trade both players sealed is final and is not reversed, even after a mistake.'
				]
			},
			{
				title: 'Shoutbox, profiles and messages',
				body: [
					'You are responsible for what you write. No harassment, hate, threats, personal information about others, spam, advertising or illegal content.',
					'You can block players: they can no longer trade with you, add you as a friend, or appear in your shoutbox.',
					'We may remove messages, bios and avatars that break these rules.'
				]
			},
			{
				title: 'Availability',
				body: [
					'The site is offered as it is, without guarantee. It can be down, lose progress after a failure, or close. We do our best to avoid it, but cannot promise it never happens.'
				]
			},
			{
				title: 'Changes to these terms',
				body: [
					'When these terms change in a way that matters, you will be asked to accept the new version the next time you play. If you do not accept it, you can export your data and delete your account.'
				]
			}
		]
	},
	privacy: {
		title: 'Privacy notice',
		intro:
			'What AnyCardz keeps about you, why, and what you can do about it. The data is handled by {operator}, reachable at {contact}.',
		sections: [
			{
				title: 'What we keep',
				body: [
					'Your account: username, email address, password (as a one-way hash we cannot read back), coins, and when you accepted the terms.',
					'Your game: cards and when you got them, unopened boosters, daily streak, coin packs claimed, trades, forge results as they change your cards.',
					'What you share: bio, avatar, shoutbox messages, friends and blocked players.',
					'Your sessions: for each device you are logged in on, its browser name (user agent), when you logged in and when it was last used.',
					'Notifications sent to you, and whether you read them.'
				]
			},
			{
				title: 'Why',
				body: [
					'To run the game you asked to play: your account, your cards, your trades. To keep it safe: sessions and failed logins are used to stop password guessing and abuse.',
					'Your email address is used to log in. We send no emails for now, and never advertising.'
				]
			},
			{
				title: 'Who sees what',
				body: [
					'Everyone playing can see your username, avatar, bio, the cards you collected and your shoutbox messages. Your email, coins, sessions and trades are private (a trade is visible to both players in it).',
					'Your data is not sold or given to anyone. It is stored on the servers that run the site.',
					'Counts without names (requests per page, packs opened, coins spent) are kept to watch the site’s health; they contain no personal data.'
				]
			},
			{
				title: 'Cookies',
				body: [
					'One cookie keeps you logged in. Two more remember your theme and language. There are no advertising or tracking cookies, and no third-party trackers.'
				]
			},
			{
				title: 'How long',
				body: [
					'As long as your account exists. A session ends 30 days after you log in, or when you log out. When you delete your account, everything listed above goes at once, including your trades and messages.'
				]
			},
			{
				title: 'Your rights',
				body: [
					'From Settings you can change your email and password, see and sign out your devices, download everything we keep about you as one file, and delete your account.',
					'For anything else (correcting data, a question, a complaint), write to {contact}. If you live in the EU, you can also complain to your data protection authority.'
				]
			}
		]
	}
};

const fr: typeof en = {
	operator: 'l’exploitant de ce site',
	contact: 'l’adresse de contact du site',
	terms: {
		title: 'Conditions d’utilisation',
		intro:
			'Ces conditions s’appliquent à toute personne qui joue à AnyCardz. En créant un compte, vous les acceptez. AnyCardz est exploité par {operator}.',
		sections: [
			{
				title: 'Le jeu',
				body: [
					'AnyCardz est un jeu de collection de cartes : vous obtenez des pièces, achetez des boosters, les ouvrez, puis collectionnez, forgez et échangez les cartes obtenues.',
					'Le jeu peut évoluer à tout moment : prix, probabilités, récompenses, collections et fonctionnalités peuvent être modifiés, ajoutés ou retirés.'
				]
			},
			{
				title: 'Les pièces et les cartes n’ont aucune valeur monétaire',
				body: [
					'Les pièces, boosters et cartes sont des objets de jeu. Ce n’est pas de l’argent, ils ne s’échangent pas contre de l’argent et ne vous appartiennent pas : vous avez le droit de les utiliser dans le jeu, rien de plus.',
					'Les lots de pièces sont gratuits pour l’instant : aucun paiement n’est demandé. Si des lots payants arrivent un jour, ils seront annoncés et ces conditions changeront d’abord.',
					'Vendre, acheter ou échanger des comptes, des pièces ou des cartes contre de l’argent réel ou quoi que ce soit hors du jeu est interdit.'
				]
			},
			{
				title: 'Votre compte',
				body: [
					'Une personne, un compte. Gardez votre mot de passe pour vous : ce qui est fait avec votre compte relève de votre responsabilité.',
					'Votre pseudo ne doit pas usurper l’identité de quelqu’un, insulter ni être obscène. Nous pouvons renommer ou fermer les comptes qui enfreignent ces conditions.',
					'Vous pouvez supprimer votre compte à tout moment depuis les Réglages. La suppression est immédiate et définitive.'
				]
			},
			{
				title: 'Jouer franc-jeu',
				body: [
					'N’utilisez ni robots, ni scripts, ni bugs pour obtenir des pièces ou des cartes, ralentir le site ou accéder aux données d’autres joueurs. Si vous trouvez un bug, signalez-le à {contact}.',
					'Utiliser plusieurs comptes pour transférer des pièces ou des cartes vers l’un d’eux est interdit.',
					'Nous pouvons retirer les objets obtenus en enfreignant ces règles, ou suspendre les comptes concernés.'
				]
			},
			{
				title: 'Échanges',
				body: [
					'Les échanges se font entre joueurs. Vérifiez chaque offre avant de la sceller : un échange scellé par les deux joueurs est définitif et n’est pas annulé, même après une erreur.'
				]
			},
			{
				title: 'Discussion, profils et messages',
				body: [
					'Vous êtes responsable de ce que vous écrivez. Pas de harcèlement, de haine, de menaces, d’informations personnelles sur d’autres, de spam, de publicité ni de contenu illégal.',
					'Vous pouvez bloquer des joueurs : ils ne peuvent plus échanger avec vous, vous ajouter en ami, ni apparaître dans votre discussion.',
					'Nous pouvons retirer les messages, bios et avatars qui enfreignent ces règles.'
				]
			},
			{
				title: 'Disponibilité',
				body: [
					'Le site est fourni tel quel, sans garantie. Il peut être indisponible, perdre de la progression après une panne, ou fermer. Nous faisons de notre mieux pour l’éviter, sans pouvoir promettre que cela n’arrive jamais.'
				]
			},
			{
				title: 'Modification de ces conditions',
				body: [
					'Si ces conditions changent de façon importante, il vous sera demandé d’accepter la nouvelle version lors de votre prochaine partie. Si vous ne l’acceptez pas, vous pouvez exporter vos données et supprimer votre compte.'
				]
			}
		]
	},
	privacy: {
		title: 'Confidentialité',
		intro:
			'Ce qu’AnyCardz conserve à votre sujet, pourquoi, et ce que vous pouvez faire. Les données sont traitées par {operator}, joignable à {contact}.',
		sections: [
			{
				title: 'Ce que nous conservons',
				body: [
					'Votre compte : pseudo, adresse e-mail, mot de passe (sous forme d’empreinte à sens unique, illisible pour nous), pièces, et la date d’acceptation des conditions.',
					'Votre jeu : cartes et date d’obtention, boosters non ouverts, série quotidienne, succès, lots de pièces obtenus, échanges, résultats de la forge quand ils modifient vos cartes.',
					'Ce que vous partagez : bio, avatar, messages de la discussion, amis et joueurs bloqués.',
					'Vos sessions : pour chaque appareil connecté, le nom de son navigateur (user agent), la date de connexion et de dernière utilisation.',
					'Les notifications qui vous sont envoyées, et si vous les avez lues.'
				]
			},
			{
				title: 'Pourquoi',
				body: [
					'Pour faire fonctionner le jeu auquel vous avez choisi de jouer : votre compte, vos cartes, vos échanges. Pour le sécuriser : les sessions et les connexions échouées servent à bloquer les tentatives de deviner un mot de passe et les abus.',
					'Votre adresse e-mail sert à vous connecter. Nous n’envoyons aucun e-mail pour l’instant, et jamais de publicité.'
				]
			},
			{
				title: 'Qui voit quoi',
				body: [
					'Tous les joueurs voient votre pseudo, votre avatar, votre bio, les cartes que vous avez collectionnées et vos messages dans la discussion. Votre e-mail, vos pièces, vos sessions et vos échanges sont privés (un échange est visible des deux joueurs concernés).',
					'Vos données ne sont ni vendues ni cédées à quiconque. Elles sont stockées sur les serveurs qui font fonctionner le site.',
					'Des comptages sans nom (requêtes par page, boosters ouverts, pièces dépensées) sont conservés pour surveiller la santé du site ; ils ne contiennent aucune donnée personnelle.'
				]
			},
			{
				title: 'Cookies',
				body: [
					'Un cookie vous garde connecté. Deux autres retiennent votre thème et votre langue. Aucun cookie publicitaire ou de pistage, aucun traceur tiers.'
				]
			},
			{
				title: 'Combien de temps',
				body: [
					'Tant que votre compte existe. Une session expire 30 jours après la connexion, ou à la déconnexion. Quand vous supprimez votre compte, tout ce qui est listé ci-dessus disparaît immédiatement, y compris vos échanges et vos messages.'
				]
			},
			{
				title: 'Vos droits',
				body: [
					'Depuis les Réglages, vous pouvez changer votre e-mail et votre mot de passe, voir et déconnecter vos appareils, télécharger en un fichier tout ce que nous conservons sur vous, et supprimer votre compte.',
					'Pour toute autre demande (rectification, question, réclamation), écrivez à {contact}. Vous pouvez aussi saisir la CNIL ou l’autorité de protection des données de votre pays.'
				]
			}
		]
	}
};

const DOCS = { en, fr };

/** The terms or the privacy notice in `lang`, with the operator filled in */
export function legalDoc(
	lang: 'en' | 'fr',
	which: 'terms' | 'privacy',
	info: LegalInfo | null
): Doc {
	const t = DOCS[lang];
	const fill = (s: string) =>
		s
			.replaceAll('{operator}', info?.operator || t.operator)
			.replaceAll('{contact}', info?.contact || t.contact);
	const doc = t[which];
	return {
		title: doc.title,
		intro: fill(doc.intro),
		sections: doc.sections.map((s) => ({ title: s.title, body: s.body.map(fill) }))
	};
}
