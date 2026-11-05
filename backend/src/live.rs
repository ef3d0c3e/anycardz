//! Live events pushed over the WebSocket: rare pulls and shoutbox messages for
//! everyone, trade changes for the two players concerned. Events carry card
//! slugs, not texts: each client shows them in its own language.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::data::Rarity;

/// Pulls kept for clients that connect later
const RECENT_PULLS: usize = 30;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PullSource {
    Pack,
    Tradeup,
    Roulette,
}

#[derive(Debug, Clone, Serialize)]
pub struct PullEvent {
    pub user: String,
    pub card: String,
    pub rarity: Rarity,
    pub foil: Option<String>,
    pub source: PullSource,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatEvent {
    pub id: i64,
    pub user: String,
    pub body: String,
    pub card: Option<String>,
    pub foil: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Pull(PullEvent),
    Chat(ChatEvent),
    /// Something changed in a trade: its players refetch it
    Trade {
        id: i64,
    },
    /// A new notification for the receiving user
    Notification(serde_json::Value),
}

#[derive(Debug)]
pub struct Envelope {
    /// Only these users receive it; None for everyone
    pub to: Option<Vec<i64>>,
    pub json: String,
}

impl Envelope {
    pub fn is_for(&self, user_id: i64) -> bool {
        self.to.as_ref().is_none_or(|to| to.contains(&user_id))
    }
}

pub struct Hub {
    tx: broadcast::Sender<Arc<Envelope>>,
    pulls: Mutex<VecDeque<PullEvent>>,
    /// Open sockets per user: a user is online while one is open
    online: Mutex<HashMap<i64, usize>>,
}

impl Default for Hub {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(512);
        Hub { tx, pulls: Mutex::new(VecDeque::new()), online: Mutex::new(HashMap::new()) }
    }
}

impl Hub {
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.tx.subscribe()
    }

    fn send(&self, to: Option<Vec<i64>>, event: &Event) {
        // No connected client is fine: the event is simply dropped
        let json = serde_json::to_string(event).expect("events serialize");
        let _ = self.tx.send(Arc::new(Envelope { to, json }));
    }

    /// A pull worth showing off: rare or better, or any foil
    pub fn pull(&self, event: PullEvent) {
        if event.rarity == Rarity::Common && event.foil.is_none() {
            return;
        }
        {
            let mut pulls = self.pulls.lock().unwrap();
            pulls.push_front(event.clone());
            pulls.truncate(RECENT_PULLS);
        }
        self.send(None, &Event::Pull(event));
    }

    pub fn chat(&self, event: ChatEvent) {
        self.send(None, &Event::Chat(event));
    }

    pub fn notify(&self, user: i64, notification: serde_json::Value) {
        self.send(Some(vec![user]), &Event::Notification(notification));
    }

    pub fn trade(&self, id: i64, players: [i64; 2]) {
        self.send(Some(players.to_vec()), &Event::Trade { id });
    }

    pub fn connected(&self, uid: i64) {
        *self.online.lock().unwrap().entry(uid).or_default() += 1;
    }

    pub fn disconnected(&self, uid: i64) {
        let mut online = self.online.lock().unwrap();
        if let Some(n) = online.get_mut(&uid) {
            *n -= 1;
            if *n == 0 {
                online.remove(&uid);
            }
        }
    }

    /// Open sockets, and the users they belong to
    pub fn counts(&self) -> (usize, usize) {
        let online = self.online.lock().unwrap();
        (online.values().sum(), online.len())
    }

    pub fn is_online(&self, uid: i64) -> bool {
        self.online.lock().unwrap().contains_key(&uid)
    }

    /// Newest first
    pub fn recent_pulls(&self) -> Vec<PullEvent> {
        self.pulls.lock().unwrap().iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pull(rarity: Rarity, foil: Option<&str>, card: &str) -> PullEvent {
        PullEvent {
            user: "a".into(),
            card: card.into(),
            rarity,
            foil: foil.map(Into::into),
            source: PullSource::Pack,
            at: Utc::now(),
        }
    }

    #[test]
    fn only_notable_pulls_are_kept_and_sent() {
        let hub = Hub::default();
        let mut rx = hub.subscribe();
        hub.pull(pull(Rarity::Common, None, "plain"));
        hub.pull(pull(Rarity::Common, Some("foil1"), "shiny"));
        hub.pull(pull(Rarity::Legendary, None, "big"));
        let first: serde_json::Value = serde_json::from_str(&rx.try_recv().unwrap().json).unwrap();
        assert_eq!(first["type"], "pull");
        assert_eq!(first["card"], "shiny");
        assert_eq!(hub.recent_pulls().iter().map(|p| p.card.as_str()).collect::<Vec<_>>(), ["big", "shiny"]);
    }

    #[test]
    fn online_while_a_socket_is_open() {
        let hub = Hub::default();
        hub.connected(1);
        hub.connected(1);
        hub.disconnected(1);
        assert!(hub.is_online(1), "a second tab is still open");
        hub.disconnected(1);
        assert!(!hub.is_online(1));
        hub.disconnected(2);
        assert!(!hub.is_online(2));
    }

    #[test]
    fn recent_pulls_are_capped() {
        let hub = Hub::default();
        for i in 0..50 {
            hub.pull(pull(Rarity::Rare, None, &i.to_string()));
        }
        let recent = hub.recent_pulls();
        assert_eq!(recent.len(), RECENT_PULLS);
        assert_eq!(recent[0].card, "49");
    }

    #[test]
    fn trade_events_reach_their_players_only() {
        let hub = Hub::default();
        let mut rx = hub.subscribe();
        hub.trade(7, [1, 2]);
        let env = rx.try_recv().unwrap();
        assert!(env.is_for(1) && env.is_for(2) && !env.is_for(3));
        assert_eq!(env.json, r#"{"type":"trade","id":7}"#);
        hub.chat(ChatEvent { id: 1, user: "a".into(), body: "hi".into(), card: None, foil: None, at: Utc::now() });
        assert!(rx.try_recv().unwrap().is_for(3), "chat goes to everyone");
    }
}
