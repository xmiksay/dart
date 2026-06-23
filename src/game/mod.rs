//! Deterministic game engines (X01, Cricket) and player management.
//!
//! These operate on [`crate::geometry::Score`] darts and are entirely
//! independent of detection, capture, and the web layer.

pub mod cricket;
pub mod x01;

use anyhow::{bail, Result};

/// Maximum darts a player throws per turn.
pub const DARTS_PER_TURN: usize = 3;

pub type PlayerId = u32;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
}

/// Ordered set of players. Turn order follows roster order.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Roster {
    players: Vec<Player>,
    next_id: PlayerId,
}

impl Roster {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a player, returning the assigned id.
    pub fn add(&mut self, name: impl Into<String>) -> PlayerId {
        let id = self.next_id;
        self.next_id += 1;
        self.players.push(Player {
            id,
            name: name.into(),
        });
        id
    }

    pub fn rename(&mut self, id: PlayerId, name: impl Into<String>) -> Result<()> {
        let p = self
            .players
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| anyhow::anyhow!("no player with id {id}"))?;
        p.name = name.into();
        Ok(())
    }

    pub fn remove(&mut self, id: PlayerId) -> Result<()> {
        let before = self.players.len();
        self.players.retain(|p| p.id != id);
        if self.players.len() == before {
            bail!("no player with id {id}");
        }
        Ok(())
    }

    /// Move a player to a new position in the turn order.
    pub fn reorder(&mut self, id: PlayerId, new_index: usize) -> Result<()> {
        if new_index >= self.players.len() {
            bail!("new_index {new_index} out of range");
        }
        let cur = self
            .players
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| anyhow::anyhow!("no player with id {id}"))?;
        let p = self.players.remove(cur);
        self.players.insert(new_index, p);
        Ok(())
    }

    pub fn players(&self) -> &[Player] {
        &self.players
    }

    pub fn get(&self, id: PlayerId) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn ids(&self) -> Vec<PlayerId> {
        self.players.iter().map(|p| p.id).collect()
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_assigns_unique_ids() {
        let mut r = Roster::new();
        let a = r.add("Ann");
        let b = r.add("Bob");
        assert_ne!(a, b);
        assert_eq!(r.len(), 2);
        assert_eq!(r.get(a).unwrap().name, "Ann");
    }

    #[test]
    fn rename_and_remove() {
        let mut r = Roster::new();
        let a = r.add("Ann");
        r.rename(a, "Anna").unwrap();
        assert_eq!(r.get(a).unwrap().name, "Anna");
        r.remove(a).unwrap();
        assert!(r.get(a).is_none());
        assert!(r.remove(a).is_err());
    }

    #[test]
    fn reorder_moves_player() {
        let mut r = Roster::new();
        let a = r.add("Ann");
        let b = r.add("Bob");
        let c = r.add("Cy");
        r.reorder(c, 0).unwrap();
        assert_eq!(r.ids(), vec![c, a, b]);
        assert!(r.reorder(a, 99).is_err());
    }
}
