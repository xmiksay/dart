//! X01 game engine (301 / 501 / 701 / 901) with configurable in/out rules.

use super::{PlayerId, Roster, TurnRecord, DARTS_PER_TURN};
use crate::geometry::Score;
use anyhow::{bail, Result};
use std::collections::HashMap;

/// How a leg may be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InRule {
    Straight,
    Double,
}

/// How a leg must be finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutRule {
    Straight,
    Double,
    Triple,
    /// Double or treble.
    Master,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct X01Config {
    pub start: u32,
    pub in_rule: InRule,
    pub out_rule: OutRule,
}

impl Default for X01Config {
    fn default() -> Self {
        Self {
            start: 501,
            in_rule: InRule::Straight,
            out_rule: OutRule::Double,
        }
    }
}

/// Result of throwing one dart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    /// Dart scored; remaining shown.
    Scored { remaining: u32 },
    /// Dart didn't count (in-rule not yet satisfied).
    NotOpened,
    /// Turn busted; remaining reverted to its start-of-turn value.
    Bust,
    /// Player checked out and won the leg.
    Win,
}

#[derive(Debug, Clone, Copy)]
struct PlayerState {
    remaining: u32,
    opened: bool,
}

pub struct X01Game {
    config: X01Config,
    order: Vec<PlayerId>,
    state: HashMap<PlayerId, PlayerState>,
    turn_idx: usize,
    /// Finalized turns in throw order; turn `i` belongs to `order[i % n]`.
    committed: Vec<Vec<Score>>,
    current_turn: Vec<Score>,
    turn_start: PlayerState,
    winner: Option<PlayerId>,
}

impl X01Game {
    pub fn new(config: X01Config, roster: &Roster) -> Result<Self> {
        if roster.is_empty() {
            bail!("cannot start an X01 game with no players");
        }
        Ok(Self::with_order(config, roster.ids()))
    }

    fn with_order(config: X01Config, order: Vec<PlayerId>) -> Self {
        let opened = config.in_rule == InRule::Straight;
        let init = PlayerState {
            remaining: config.start,
            opened,
        };
        let state = order.iter().map(|&id| (id, init)).collect();
        Self {
            config,
            order,
            state,
            turn_idx: 0,
            committed: Vec::new(),
            current_turn: Vec::new(),
            turn_start: init,
            winner: None,
        }
    }

    pub fn current_player(&self) -> PlayerId {
        self.order[self.turn_idx]
    }

    pub fn remaining(&self, id: PlayerId) -> Option<u32> {
        self.state.get(&id).map(|s| s.remaining)
    }

    pub fn winner(&self) -> Option<PlayerId> {
        self.winner
    }

    pub fn current_turn(&self) -> &[Score] {
        &self.current_turn
    }

    /// Apply one dart for the current player.
    pub fn throw(&mut self, dart: Score) -> Result<Outcome> {
        if self.winner.is_some() {
            bail!("game already won");
        }
        let id = self.current_player();
        let mut st = self.state[&id];

        let counts = st.opened || self.opens(&dart);
        if !counts {
            self.current_turn.push(dart);
            self.maybe_end_turn();
            return Ok(Outcome::NotOpened);
        }
        st.opened = true;

        let new = st.remaining as i64 - dart.value as i64;
        let outcome = if new < 0 || self.is_bust_floor(new) {
            Outcome::Bust
        } else if new == 0 {
            if self.valid_finish(&dart) {
                Outcome::Win
            } else {
                Outcome::Bust
            }
        } else {
            Outcome::Scored {
                remaining: new as u32,
            }
        };

        match outcome {
            Outcome::Bust => {
                self.state.insert(id, self.turn_start);
                self.current_turn.push(dart);
                self.advance();
                Ok(Outcome::Bust)
            }
            Outcome::Win => {
                st.remaining = 0;
                self.state.insert(id, st);
                self.current_turn.push(dart);
                self.winner = Some(id);
                Ok(Outcome::Win)
            }
            Outcome::Scored { remaining } => {
                st.remaining = remaining;
                self.state.insert(id, st);
                self.current_turn.push(dart);
                self.maybe_end_turn();
                Ok(Outcome::Scored { remaining })
            }
            Outcome::NotOpened => unreachable!(),
        }
    }

    /// All turns (finalized + the in-progress/winning one), oldest first.
    pub fn turns(&self) -> Vec<TurnRecord> {
        let n = self.order.len();
        let mut out: Vec<TurnRecord> = self
            .committed
            .iter()
            .enumerate()
            .map(|(i, darts)| TurnRecord {
                player: self.order[i % n],
                darts: darts.clone(),
            })
            .collect();
        if !self.current_turn.is_empty() {
            out.push(TurnRecord {
                player: self.current_player(),
                darts: self.current_turn.clone(),
            });
        }
        out
    }

    /// The whole leg's darts in throw order (finalized turns then the live turn).
    fn flat(&self) -> Vec<Score> {
        self.committed
            .iter()
            .flatten()
            .copied()
            .chain(self.current_turn.iter().copied())
            .collect()
    }

    /// Rebuild all derived state by replaying a dart log from the start. This is
    /// how edits/undo stay correct: any change to history is a replay.
    fn rebuild(&mut self, flat: Vec<Score>) {
        let mut g = Self::with_order(self.config, self.order.clone());
        for d in flat {
            if g.winner.is_some() {
                break; // darts after a (possibly newly-earlier) checkout can't be thrown
            }
            let _ = g.throw(d);
        }
        *self = g;
    }

    /// Remove the most recent dart anywhere in the leg — including reopening the
    /// previous turn once the current one has been committed. Manual-correction
    /// primitive; works after a win (reopens the leg).
    pub fn undo_last_dart(&mut self) -> Result<Option<Score>> {
        let mut flat = self.flat();
        let removed = flat.pop();
        if removed.is_some() {
            self.rebuild(flat);
        }
        Ok(removed)
    }

    /// Correct a single dart in any turn (0-based turn then dart index) and
    /// recompute the leg by replay. Lets the operator fix a misread after the
    /// turn has closed.
    pub fn edit_dart(&mut self, turn_index: usize, dart_index: usize, dart: Score) -> Result<()> {
        let turns = self.turns();
        let turn = turns
            .get(turn_index)
            .ok_or_else(|| anyhow::anyhow!("no turn at index {turn_index}"))?;
        if dart_index >= turn.darts.len() {
            bail!("turn {turn_index} has no dart at index {dart_index}");
        }
        let before: usize = turns[..turn_index].iter().map(|t| t.darts.len()).sum();
        let flat_index = before + dart_index;
        let mut flat = self.flat();
        flat[flat_index] = dart;
        self.rebuild(flat);
        Ok(())
    }

    /// Suggest a checkout for `remaining` under the configured out rule, if one
    /// exists within three darts. Returns the dart sequence.
    pub fn suggest_checkout(&self, remaining: u32) -> Option<Vec<Score>> {
        suggest_checkout(remaining, self.config.out_rule)
    }

    fn opens(&self, dart: &Score) -> bool {
        match self.config.in_rule {
            InRule::Straight => true,
            InRule::Double => dart.is_double(),
        }
    }

    fn valid_finish(&self, dart: &Score) -> bool {
        match self.config.out_rule {
            OutRule::Straight => true,
            OutRule::Double => dart.is_double(),
            OutRule::Triple => dart.is_treble(),
            OutRule::Master => dart.is_double() || dart.is_treble(),
        }
    }

    /// A positive remaining too small to finish under the out rule is a bust.
    fn is_bust_floor(&self, new: i64) -> bool {
        if new <= 0 {
            return false;
        }
        let floor = match self.config.out_rule {
            OutRule::Straight => 1,
            OutRule::Double | OutRule::Master => 2,
            OutRule::Triple => 3,
        };
        new < floor
    }

    fn maybe_end_turn(&mut self) {
        if self.current_turn.len() >= DARTS_PER_TURN {
            self.advance();
        }
    }

    fn advance(&mut self) {
        // Preserve the finished turn so it can be shown and edited later.
        self.committed.push(std::mem::take(&mut self.current_turn));
        self.turn_idx = (self.turn_idx + 1) % self.order.len();
        self.turn_start = self.state[&self.current_player()];
    }
}

/// All single-throw scores a dart can produce, ordered for "nice" suggestions
/// (high trebles first, then bulls, singles, doubles).
fn all_darts() -> Vec<Score> {
    let mut v = Vec::new();
    for n in (1..=20).rev() {
        v.push(Score::treble(n));
    }
    v.push(Score::double_bull());
    v.push(Score::bull());
    for n in (1..=20).rev() {
        v.push(Score::single(n));
    }
    for n in (1..=20).rev() {
        v.push(Score::double(n));
    }
    v
}

fn finishers(out_rule: OutRule) -> Vec<Score> {
    all_darts()
        .into_iter()
        .filter(|d| match out_rule {
            OutRule::Straight => true,
            OutRule::Double => d.is_double(),
            OutRule::Triple => d.is_treble(),
            OutRule::Master => d.is_double() || d.is_treble(),
        })
        .collect()
}

/// Find a ≤3-dart checkout for `remaining`, last dart satisfying `out_rule`.
pub fn suggest_checkout(remaining: u32, out_rule: OutRule) -> Option<Vec<Score>> {
    if remaining == 0 || remaining > 170 {
        return None;
    }
    let setup = all_darts();
    let fin = finishers(out_rule);

    // 1 dart.
    for f in &fin {
        if f.value == remaining {
            return Some(vec![*f]);
        }
    }
    // 2 darts.
    for d1 in &setup {
        for f in &fin {
            if d1.value + f.value == remaining {
                return Some(vec![*d1, *f]);
            }
        }
    }
    // 3 darts.
    for d1 in &setup {
        for d2 in &setup {
            for f in &fin {
                if d1.value + d2.value + f.value == remaining {
                    return Some(vec![*d1, *d2, *f]);
                }
            }
        }
    }
    None
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

    fn cfg(start: u32, out: OutRule) -> X01Config {
        X01Config {
            start,
            in_rule: InRule::Straight,
            out_rule: out,
        }
    }

    #[test]
    fn straight_scoring_subtracts() {
        let r = roster2();
        let mut g = X01Game::new(cfg(501, OutRule::Straight), &r).unwrap();
        assert_eq!(
            g.throw(Score::treble(20)).unwrap(),
            Outcome::Scored { remaining: 441 }
        );
    }

    #[test]
    fn turn_advances_after_three_darts() {
        let r = roster2();
        let ids = r.ids();
        let mut g = X01Game::new(cfg(501, OutRule::Straight), &r).unwrap();
        assert_eq!(g.current_player(), ids[0]);
        for _ in 0..3 {
            g.throw(Score::single(1)).unwrap();
        }
        assert_eq!(g.current_player(), ids[1]);
    }

    #[test]
    fn double_out_win() {
        let r = roster2();
        let mut g = X01Game::new(cfg(40, OutRule::Double), &r).unwrap();
        assert_eq!(g.throw(Score::double(20)).unwrap(), Outcome::Win);
        assert_eq!(g.winner(), Some(r.ids()[0]));
    }

    #[test]
    fn reaching_zero_without_double_is_bust() {
        let r = roster2();
        let ids = r.ids();
        let mut g = X01Game::new(cfg(40, OutRule::Double), &r).unwrap();
        assert_eq!(
            g.throw(Score::single(20)).unwrap(),
            Outcome::Scored { remaining: 20 }
        );
        // 20 left, single 20 -> 0 but not a double => bust, reverts to 40, turn ends.
        assert_eq!(g.throw(Score::single(20)).unwrap(), Outcome::Bust);
        assert_eq!(g.remaining(ids[0]), Some(40));
        assert_eq!(g.current_player(), ids[1]);
    }

    #[test]
    fn overshoot_is_bust_and_reverts() {
        let r = roster2();
        let ids = r.ids();
        let mut g = X01Game::new(cfg(20, OutRule::Double), &r).unwrap();
        assert_eq!(g.throw(Score::treble(20)).unwrap(), Outcome::Bust);
        assert_eq!(g.remaining(ids[0]), Some(20));
    }

    #[test]
    fn leaving_one_on_double_out_is_bust() {
        let r = roster2();
        let mut g = X01Game::new(cfg(20, OutRule::Double), &r).unwrap();
        // 20 - 19 = 1 -> impossible to finish on a double => bust.
        assert_eq!(g.throw(Score::single(19)).unwrap(), Outcome::Bust);
    }

    #[test]
    fn double_in_ignores_until_double() {
        let r = roster2();
        let cfg = X01Config {
            start: 501,
            in_rule: InRule::Double,
            out_rule: OutRule::Double,
        };
        let mut g = X01Game::new(cfg, &r).unwrap();
        assert_eq!(g.throw(Score::single(20)).unwrap(), Outcome::NotOpened);
        assert_eq!(g.remaining(r.ids()[0]), Some(501));
        assert_eq!(
            g.throw(Score::double(20)).unwrap(),
            Outcome::Scored { remaining: 461 }
        );
    }

    #[test]
    fn undo_restores_remaining() {
        let r = roster2();
        let id = r.ids()[0];
        let mut g = X01Game::new(cfg(501, OutRule::Double), &r).unwrap();
        g.throw(Score::treble(20)).unwrap();
        g.throw(Score::treble(19)).unwrap();
        assert_eq!(g.remaining(id), Some(501 - 60 - 57));
        let removed = g.undo_last_dart().unwrap().unwrap();
        assert_eq!(removed, Score::treble(19));
        assert_eq!(g.remaining(id), Some(501 - 60));
        assert_eq!(g.current_turn().len(), 1);
    }

    #[test]
    fn checkout_170_is_two_trebles_and_bull() {
        let co = suggest_checkout(170, OutRule::Double).unwrap();
        assert_eq!(co.iter().map(|d| d.value).sum::<u32>(), 170);
        assert!(co.last().unwrap().is_double());
        // 170 = T20, T20, Bull(50).
        assert_eq!(co.len(), 3);
    }

    #[test]
    fn checkout_40_is_double_20() {
        let co = suggest_checkout(40, OutRule::Double).unwrap();
        assert_eq!(co, vec![Score::double(20)]);
    }

    #[test]
    fn no_checkout_above_170() {
        assert!(suggest_checkout(171, OutRule::Double).is_none());
    }

    #[test]
    fn undo_reopens_previous_turn() {
        let r = roster2();
        let ids = r.ids();
        let mut g = X01Game::new(cfg(501, OutRule::Straight), &r).unwrap();
        for _ in 0..3 {
            g.throw(Score::single(5)).unwrap();
        }
        assert_eq!(g.current_player(), ids[1]); // p0's turn closed
        let removed = g.undo_last_dart().unwrap().unwrap();
        assert_eq!(removed, Score::single(5));
        assert_eq!(g.current_player(), ids[0]); // reopened
        assert_eq!(g.remaining(ids[0]), Some(501 - 10));
        assert_eq!(g.current_turn().len(), 2);
    }

    #[test]
    fn edit_past_turn_recomputes() {
        let r = roster2();
        let ids = r.ids();
        let mut g = X01Game::new(cfg(501, OutRule::Straight), &r).unwrap();
        for _ in 0..3 {
            g.throw(Score::single(20)).unwrap(); // p0 -> 441
        }
        g.throw(Score::single(1)).unwrap(); // p1 -> 500
                                            // Correct p0's first dart (turn 0, dart 0) from single-20 to treble-20.
        g.edit_dart(0, 0, Score::treble(20)).unwrap();
        assert_eq!(g.remaining(ids[0]), Some(501 - 60 - 20 - 20));
        assert_eq!(g.remaining(ids[1]), Some(500)); // p1 untouched
        let turns = g.turns();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].player, ids[0]);
        assert_eq!(turns[0].darts.len(), 3);
        assert_eq!(turns[1].player, ids[1]);
        assert_eq!(g.current_player(), ids[1]);
    }

    #[test]
    fn edit_rejects_out_of_range() {
        let r = roster2();
        let mut g = X01Game::new(cfg(501, OutRule::Double), &r).unwrap();
        g.throw(Score::single(20)).unwrap();
        assert!(g.edit_dart(5, 0, Score::single(1)).is_err());
        assert!(g.edit_dart(0, 9, Score::single(1)).is_err());
    }
}
