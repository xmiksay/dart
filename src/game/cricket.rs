//! Cricket game engine: numbers 15–20 plus bull, with optional cut-throat.

use super::{PlayerId, Roster, DARTS_PER_TURN};
use crate::geometry::{Ring, Score};
use anyhow::{bail, Result};
use std::collections::HashMap;

/// Scoring targets in cricket. 25 represents the bull.
pub const NUMBERS: [u8; 7] = [20, 19, 18, 17, 16, 15, 25];

#[derive(Debug, Clone, Default)]
struct PlayerState {
    /// Marks per number, capped at 3 (3 = closed).
    marks: HashMap<u8, u8>,
    points: u32,
}

impl PlayerState {
    fn marks_on(&self, n: u8) -> u8 {
        *self.marks.get(&n).unwrap_or(&0)
    }
    fn closed_all(&self) -> bool {
        NUMBERS.iter().all(|n| self.marks_on(*n) >= 3)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    /// Dart landed on a cricket number.
    Marked {
        number: u8,
        marks_added: u8,
        points_scored: u32,
    },
    /// Dart missed or hit a non-cricket number.
    NoMark,
    /// This dart won the game.
    Win,
}

pub struct CricketGame {
    cut_throat: bool,
    order: Vec<PlayerId>,
    state: HashMap<PlayerId, PlayerState>,
    turn_idx: usize,
    current_turn: Vec<Score>,
    /// Full-state snapshot at the start of the current turn, for undo.
    turn_snapshot: HashMap<PlayerId, PlayerState>,
    winner: Option<PlayerId>,
}

impl CricketGame {
    pub fn new(roster: &Roster, cut_throat: bool) -> Result<Self> {
        if roster.is_empty() {
            bail!("cannot start a cricket game with no players");
        }
        let order = roster.ids();
        let state: HashMap<_, _> = order
            .iter()
            .map(|&id| (id, PlayerState::default()))
            .collect();
        Ok(Self {
            cut_throat,
            order,
            state: state.clone(),
            turn_idx: 0,
            current_turn: Vec::new(),
            turn_snapshot: state,
            winner: None,
        })
    }

    pub fn current_player(&self) -> PlayerId {
        self.order[self.turn_idx]
    }

    pub fn points(&self, id: PlayerId) -> Option<u32> {
        self.state.get(&id).map(|s| s.points)
    }

    pub fn marks(&self, id: PlayerId, number: u8) -> Option<u8> {
        self.state.get(&id).map(|s| s.marks_on(number))
    }

    pub fn winner(&self) -> Option<PlayerId> {
        self.winner
    }

    pub fn current_turn(&self) -> &[Score] {
        &self.current_turn
    }

    pub fn throw(&mut self, dart: Score) -> Result<Outcome> {
        if self.winner.is_some() {
            bail!("game already won");
        }
        let (number, marks_added) = match dart_target(&dart) {
            Some(t) => t,
            None => {
                self.current_turn.push(dart);
                self.maybe_end_turn();
                return Ok(Outcome::NoMark);
            }
        };

        let id = self.current_player();
        let cur = self.state[&id].marks_on(number);
        let toward_close = marks_added.min(3 - cur);
        let overflow = marks_added - toward_close;

        // Apply marks.
        self.state
            .get_mut(&id)
            .unwrap()
            .marks
            .insert(number, cur + toward_close);

        // Score overflow marks if the number is still open for an opponent.
        let mut points_scored = 0;
        if overflow > 0 && self.number_open_for_opponents(id, number) {
            let value = number as u32;
            for _ in 0..overflow {
                points_scored += value;
            }
            if self.cut_throat {
                // Points go to every opponent who has NOT closed the number.
                let opponents: Vec<PlayerId> = self
                    .order
                    .iter()
                    .copied()
                    .filter(|&o| o != id && self.state[&o].marks_on(number) < 3)
                    .collect();
                for o in opponents {
                    self.state.get_mut(&o).unwrap().points += points_scored;
                }
            } else {
                self.state.get_mut(&id).unwrap().points += points_scored;
            }
        }

        self.current_turn.push(dart);

        if self.check_win(id) {
            self.winner = Some(id);
            return Ok(Outcome::Win);
        }

        let out = Outcome::Marked {
            number,
            marks_added,
            points_scored,
        };
        self.maybe_end_turn();
        Ok(out)
    }

    /// Remove the last dart of the in-progress turn, restoring state from the
    /// turn snapshot and replaying the remaining darts. Manual-correction primitive.
    pub fn undo_last_dart(&mut self) -> Result<Option<Score>> {
        if self.current_turn.is_empty() {
            return Ok(None);
        }
        if self.winner.is_some() {
            bail!("cannot undo after the game is won");
        }
        let removed = self.current_turn.pop();
        let darts = std::mem::take(&mut self.current_turn);
        self.state = self.turn_snapshot.clone();
        for d in &darts {
            let _ = self.throw(*d);
        }
        Ok(removed)
    }

    /// A number scores only while at least one opponent has not closed it.
    fn number_open_for_opponents(&self, id: PlayerId, number: u8) -> bool {
        self.order
            .iter()
            .any(|&o| o != id && self.state[&o].marks_on(number) < 3)
    }

    fn check_win(&self, id: PlayerId) -> bool {
        let me = &self.state[&id];
        if !me.closed_all() {
            return false;
        }
        let others = self
            .order
            .iter()
            .filter(|&&o| o != id)
            .map(|o| self.state[o].points);
        if self.cut_throat {
            others.min().map(|m| me.points <= m).unwrap_or(true)
        } else {
            others.max().map(|m| me.points >= m).unwrap_or(true)
        }
    }

    fn maybe_end_turn(&mut self) {
        if self.current_turn.len() >= DARTS_PER_TURN {
            self.current_turn.clear();
            self.turn_idx = (self.turn_idx + 1) % self.order.len();
            self.turn_snapshot = self.state.clone();
        }
    }
}

/// Map a dart to (cricket number, marks) or `None` if it isn't a cricket hit.
fn dart_target(dart: &Score) -> Option<(u8, u8)> {
    match dart.ring {
        Ring::InnerBull => Some((25, 2)),
        Ring::OuterBull => Some((25, 1)),
        Ring::Single if (15..=20).contains(&dart.segment) => Some((dart.segment, 1)),
        Ring::Double if (15..=20).contains(&dart.segment) => Some((dart.segment, 2)),
        Ring::Treble if (15..=20).contains(&dart.segment) => Some((dart.segment, 3)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster2() -> Roster {
        let mut r = Roster::new();
        r.add("A");
        r.add("B");
        r
    }

    #[test]
    fn treble_opens_a_number() {
        let r = roster2();
        let id = r.ids()[0];
        let mut g = CricketGame::new(&r, false).unwrap();
        let out = g.throw(Score::treble(20)).unwrap();
        assert_eq!(
            out,
            Outcome::Marked {
                number: 20,
                marks_added: 3,
                points_scored: 0
            }
        );
        assert_eq!(g.marks(id, 20), Some(3));
    }

    #[test]
    fn overflow_scores_points_when_open() {
        let r = roster2();
        let id = r.ids()[0];
        let mut g = CricketGame::new(&r, false).unwrap();
        g.throw(Score::treble(20)).unwrap(); // close 20
        let out = g.throw(Score::treble(20)).unwrap(); // 3 overflow marks
        assert_eq!(
            out,
            Outcome::Marked {
                number: 20,
                marks_added: 3,
                points_scored: 60
            }
        );
        assert_eq!(g.points(id), Some(60));
    }

    #[test]
    fn no_points_once_all_closed() {
        let mut r = Roster::new();
        let a = r.add("A");
        let b = r.add("B");
        let mut g = CricketGame::new(&r, false).unwrap();
        // A closes 20.
        g.throw(Score::treble(20)).unwrap();
        // advance to B (two more darts).
        g.throw(Score::miss()).unwrap();
        g.throw(Score::miss()).unwrap();
        assert_eq!(g.current_player(), b);
        // B closes 20 too -> now closed by everyone.
        g.throw(Score::treble(20)).unwrap();
        g.throw(Score::miss()).unwrap();
        g.throw(Score::miss()).unwrap();
        // Back to A: extra 20s should not score (closed for all).
        assert_eq!(g.current_player(), a);
        // marks_added reports the dart's mark value (3 for a treble); none score
        // because the number is closed for everyone.
        let out = g.throw(Score::treble(20)).unwrap();
        assert_eq!(
            out,
            Outcome::Marked {
                number: 20,
                marks_added: 3,
                points_scored: 0
            }
        );
        assert_eq!(g.points(a), Some(0));
    }

    #[test]
    fn cut_throat_gives_points_to_opponents() {
        let mut r = Roster::new();
        let a = r.add("A");
        let b = r.add("B");
        let mut g = CricketGame::new(&r, true).unwrap();
        g.throw(Score::treble(20)).unwrap(); // A closes 20
        g.throw(Score::treble(20)).unwrap(); // overflow -> opponents get points
        assert_eq!(g.points(a), Some(0));
        assert_eq!(g.points(b), Some(60));
    }

    #[test]
    fn undo_restores_marks_and_points() {
        let r = roster2();
        let id = r.ids()[0];
        let mut g = CricketGame::new(&r, false).unwrap();
        g.throw(Score::treble(20)).unwrap();
        g.throw(Score::treble(20)).unwrap();
        assert_eq!(g.points(id), Some(60));
        g.undo_last_dart().unwrap();
        assert_eq!(g.points(id), Some(0));
        assert_eq!(g.marks(id, 20), Some(3));
        assert_eq!(g.current_turn().len(), 1);
    }
}
