use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

/// Qui est connecté, et par combien de fenêtres.
///
/// **En mémoire, donc par dyno.** Avec un seul dyno — ce que Plum a — c'est
/// complet : tout le monde est sur la même instance. Avec plusieurs, deux
/// personnes tombées sur des dynos différents ne se verraient pas écrire ;
/// il faudra alors relayer par Redis, déjà provisionné pour ça.
///
/// C'est écrit ici plutôt que découvert plus tard : une limite nommée est une
/// décision, une limite tue est un bug qui attend.
#[derive(Clone, Default)]
pub struct Hub {
    connections: Arc<Mutex<HashMap<Uuid, Vec<Connection>>>>,
}

#[derive(Clone)]
struct Connection {
    id: Uuid,
    sender: UnboundedSender<String>,
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre une fenêtre et rend son identifiant, à rendre en partant.
    pub fn join(&self, user: Uuid, sender: UnboundedSender<String>) -> Uuid {
        let id = Uuid::new_v4();
        let mut guard = self.connections.lock().expect("hub empoisonné");
        guard
            .entry(user)
            .or_default()
            .push(Connection { id, sender });
        id
    }

    pub fn leave(&self, user: Uuid, connection: Uuid) {
        let mut guard = self.connections.lock().expect("hub empoisonné");
        if let Some(list) = guard.get_mut(&user) {
            list.retain(|c| c.id != connection);
            if list.is_empty() {
                // Sans ça, la table garde une entrée vide par personne ayant
                // ouvert l'application depuis le démarrage du dyno.
                guard.remove(&user);
            }
        }
    }

    /// Envoie à toutes les fenêtres d'une personne.
    ///
    /// Silencieux si elle n'est pas là : un événement en direct est un bonus,
    /// jamais le canal principal. Ce qui compte est déjà dans la base, et le
    /// client le relira.
    pub fn send(&self, user: Uuid, event: &impl Serialize) {
        let Ok(payload) = serde_json::to_string(event) else {
            tracing::error!("événement non sérialisable, non envoyé");
            return;
        };

        let mut guard = self.connections.lock().expect("hub empoisonné");
        if let Some(list) = guard.get_mut(&user) {
            // Une fenêtre dont le récepteur est parti ne reviendra pas :
            // l'écriture échoue et on l'oublie, sinon la liste grossit à
            // chaque reconnexion.
            list.retain(|c| c.sender.send(payload.clone()).is_ok());
            if list.is_empty() {
                guard.remove(&user);
            }
        }
    }

    #[cfg(test)]
    pub fn connections_for(&self, user: Uuid) -> usize {
        self.connections
            .lock()
            .expect("hub empoisonné")
            .get(&user)
            .map_or(0, Vec::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    #[derive(Serialize)]
    struct Ping {
        r#type: &'static str,
    }

    #[test]
    fn an_event_reaches_every_window_of_the_same_person() {
        let hub = Hub::new();
        let user = Uuid::new_v4();
        let (tx1, mut rx1) = unbounded_channel();
        let (tx2, mut rx2) = unbounded_channel();
        hub.join(user, tx1);
        hub.join(user, tx2);

        hub.send(user, &Ping { r#type: "ping" });

        assert_eq!(rx1.try_recv().unwrap(), r#"{"type":"ping"}"#);
        assert_eq!(rx2.try_recv().unwrap(), r#"{"type":"ping"}"#);
    }

    #[test]
    fn nothing_is_sent_to_someone_who_is_not_connected() {
        let hub = Hub::new();
        // Ne doit pas paniquer : l'absence est le cas courant.
        hub.send(Uuid::new_v4(), &Ping { r#type: "ping" });
    }

    #[test]
    fn leaving_removes_only_that_window() {
        let hub = Hub::new();
        let user = Uuid::new_v4();
        let (tx1, _rx1) = unbounded_channel();
        let (tx2, mut rx2) = unbounded_channel();
        let first = hub.join(user, tx1);
        hub.join(user, tx2);

        hub.leave(user, first);
        assert_eq!(hub.connections_for(user), 1);

        hub.send(user, &Ping { r#type: "ping" });
        assert!(rx2.try_recv().is_ok(), "l'autre fenêtre reçoit toujours");
    }

    /// Sans ça, la table garderait une entrée par personne ayant ouvert
    /// l'application depuis le démarrage du dyno.
    #[test]
    fn a_closed_receiver_is_forgotten_rather_than_kept_forever() {
        let hub = Hub::new();
        let user = Uuid::new_v4();
        let (tx, rx) = unbounded_channel();
        hub.join(user, tx);
        drop(rx);

        hub.send(user, &Ping { r#type: "ping" });
        assert_eq!(hub.connections_for(user), 0);
    }
}
