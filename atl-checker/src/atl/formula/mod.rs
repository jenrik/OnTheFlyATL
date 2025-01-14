pub mod game_formula;
pub mod parser;

use crate::atl::common::{Player, Proposition};
use crate::atl::formula::game_formula::GamePhi;
pub use crate::atl::formula::parser::*;
use crate::atl::gamestructure::GameStructure;
use joinery::prelude::*;
use std::cmp::max;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

/// Alternating-time Temporal Logic formula
#[derive(Hash, Eq, PartialEq, Clone, Debug, Deserialize, Serialize)]
pub enum Phi {
    /// Trivially satisfied
    #[serde(rename = "true")]
    True,
    /// Trivially not satisfied
    #[serde(rename = "false")]
    False,
    /// The current state must have the label/proposition
    #[serde(rename = "proposition")]
    Proposition(Proposition),
    /// It must not be the case that subformula is satisfied
    #[serde(rename = "not")]
    Not(Arc<Phi>),
    /// It must be the case that either formula is satisfied
    #[serde(rename = "or")]
    Or(Arc<Phi>, Arc<Phi>),
    /// It must be the case that either formula is satisfied
    #[serde(rename = "and")]
    And(Arc<Phi>, Arc<Phi>),
    /// It must be the case that `formula` is satisfied in the next step despite what actions `players` choose.
    #[serde(rename = "despite next")]
    DespiteNext {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
    /// It must be the case that players can enforce that `formula` is satisfied in the next step
    #[serde(rename = "enforce next")]
    EnforceNext {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
    /// It must be the case that `pre` is satisfied until `until` is satisfied despite what actions `players` choose.
    #[serde(rename = "despite until")]
    DespiteUntil {
        players: Vec<Player>,
        pre: Arc<Phi>,
        until: Arc<Phi>,
    },
    /// It must be the case that `players` can enforce that `pre` is satisfied until `until` is satisfied
    #[serde(rename = "enforce until")]
    EnforceUntil {
        players: Vec<Player>,
        pre: Arc<Phi>,
        until: Arc<Phi>,
    },
    /// It must be the case that `formula` is satisfied in some coming step despite what actions `players` choose.
    #[serde(rename = "despite eventually")]
    DespiteEventually {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
    /// It must be the case that `players` can enforce that `formula` is satisfied in some coming step.
    #[serde(rename = "enforce eventually")]
    EnforceEventually {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
    /// It must be the case that `formula` is continually satisfied despite what actions `players` choose.
    #[serde(rename = "despite invariant")]
    DespiteInvariant {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
    /// It must be the case that `players` can enforce that `formula` is continually satisfied.
    #[serde(rename = "enforce invariant")]
    EnforceInvariant {
        players: Vec<Player>,
        formula: Arc<Phi>,
    },
}

impl Phi {
    /// Returns the size of the formula. This is equivalent to the number of nodes in the
    /// phi structure
    pub fn size(&self) -> u32 {
        match self {
            Phi::True => 1,
            Phi::False => 1,
            Phi::Proposition(_) => 1,
            Phi::Not(formula) => formula.size() + 1,
            Phi::Or(formula1, formula2) => formula1.size() + formula2.size() + 1,
            Phi::And(formula1, formula2) => formula1.size() + formula2.size() + 1,
            Phi::DespiteNext { formula, .. } => formula.size() + 1,
            Phi::EnforceNext { formula, .. } => formula.size() + 1,
            Phi::DespiteUntil { pre, until, .. } => pre.size() + until.size() + 1,
            Phi::EnforceUntil { pre, until, .. } => pre.size() + until.size() + 1,
            Phi::DespiteEventually { formula, .. } => formula.size() + 1,
            Phi::EnforceEventually { formula, .. } => formula.size() + 1,
            Phi::DespiteInvariant { formula, .. } => formula.size() + 1,
            Phi::EnforceInvariant { formula, .. } => formula.size() + 1,
        }
    }

    /// Returns the depth of the formula. This is equivalent to the longest branch in the
    /// phi structure
    pub fn depth(&self) -> u32 {
        match self {
            Phi::True => 1,
            Phi::False => 1,
            Phi::Proposition(_) => 1,
            Phi::Not(formula) => formula.size() + 1,
            Phi::Or(formula1, formula2) => max(formula1.depth(), formula2.depth()) + 1,
            Phi::And(formula1, formula2) => max(formula1.depth(), formula2.depth()) + 1,
            Phi::DespiteNext { formula, .. } => formula.depth() + 1,
            Phi::EnforceNext { formula, .. } => formula.depth() + 1,
            Phi::DespiteUntil { pre, until, .. } => max(pre.depth(), until.depth()) + 1,
            Phi::EnforceUntil { pre, until, .. } => max(pre.depth(), until.depth()) + 1,
            Phi::DespiteEventually { formula, .. } => formula.depth() + 1,
            Phi::EnforceEventually { formula, .. } => formula.depth() + 1,
            Phi::DespiteInvariant { formula, .. } => formula.depth() + 1,
            Phi::EnforceInvariant { formula, .. } => formula.depth() + 1,
        }
    }

    /// Returns the number of path qualifiers in the formula. E.g.
    /// * `p & q` returns 0
    /// * `<<p1>> F s` returns 1
    /// * `(<<p1>> F s) | (<<p2>> F t)` returns 2
    /// * `<<>> G ([[p1]] F s)` returns 2
    pub fn path_qualifier_count(&self) -> u32 {
        match self {
            Phi::True => 0,
            Phi::False => 0,
            Phi::Proposition(_) => 0,
            Phi::Not(formula) => formula.path_qualifier_count(),
            Phi::Or(formula1, formula2) => {
                formula1.path_qualifier_count() + formula2.path_qualifier_count()
            }
            Phi::And(formula1, formula2) => {
                formula1.path_qualifier_count() + formula2.path_qualifier_count()
            }
            Phi::DespiteNext { formula, .. } => formula.path_qualifier_count() + 1,
            Phi::EnforceNext { formula, .. } => formula.path_qualifier_count() + 1,
            Phi::DespiteUntil { pre, until, .. } => {
                pre.path_qualifier_count() + until.path_qualifier_count() + 1
            }
            Phi::EnforceUntil { pre, until, .. } => {
                pre.path_qualifier_count() + until.path_qualifier_count() + 1
            }
            Phi::DespiteEventually { formula, .. } => formula.path_qualifier_count() + 1,
            Phi::EnforceEventually { formula, .. } => formula.path_qualifier_count() + 1,
            Phi::DespiteInvariant { formula, .. } => formula.path_qualifier_count() + 1,
            Phi::EnforceInvariant { formula, .. } => formula.path_qualifier_count() + 1,
        }
    }

    /// Returns the biggest number of nested path qualifiers in the formula. E.g.
    /// * `p & q` returns 0
    /// * `<<p1>> F s` returns 1
    /// * `(<<p1>> F s) | (<<p2>> F t)` returns 1
    /// * `<<>> G ([[p1]] F s)` returns 2
    pub fn path_qualifier_depth(&self) -> u32 {
        match self {
            Phi::True => 0,
            Phi::False => 0,
            Phi::Proposition(_) => 0,
            Phi::Not(formula) => formula.path_qualifier_depth(),
            Phi::Or(formula1, formula2) => max(
                formula1.path_qualifier_depth(),
                formula2.path_qualifier_depth(),
            ),
            Phi::And(formula1, formula2) => max(
                formula1.path_qualifier_depth(),
                formula2.path_qualifier_depth(),
            ),
            Phi::DespiteNext { formula, .. } => formula.path_qualifier_depth() + 1,
            Phi::EnforceNext { formula, .. } => formula.path_qualifier_depth() + 1,
            Phi::DespiteUntil { pre, until, .. } => {
                max(pre.path_qualifier_depth(), until.path_qualifier_depth()) + 1
            }
            Phi::EnforceUntil { pre, until, .. } => {
                max(pre.path_qualifier_depth(), until.path_qualifier_depth()) + 1
            }
            Phi::DespiteEventually { formula, .. } => formula.path_qualifier_depth() + 1,
            Phi::EnforceEventually { formula, .. } => formula.path_qualifier_depth() + 1,
            Phi::DespiteInvariant { formula, .. } => formula.path_qualifier_depth() + 1,
            Phi::EnforceInvariant { formula, .. } => formula.path_qualifier_depth() + 1,
        }
    }

    /// Pairs an ATL formula with its game structure, allowing us to print
    /// the formula using the names of players and labels that is defined by the game structure.
    /// # Example
    /// Given a Phi `formula` (`<<p1>> G p1.alive`) and a IntermediateLCGS `game_Structure`, you can
    /// create a GamePhi with `formula.in_context_of(&game_structure)`. Using this in a print statement
    /// like
    /// ```ignore
    /// println!("{}", formula.in_context_of(&game_structure))
    /// ```
    /// will print "`<<p1>> G p1.alive`" as opposed to "`<<0>> G 1`", which happens when you just write
    /// ```ignore
    /// println!("{}", formula)
    /// ```
    pub fn in_context_of<'a, G: GameStructure>(&'a self, game_structure: &'a G) -> GamePhi<'a, G> {
        GamePhi {
            formula: self,
            game: game_structure,
        }
    }
}

impl Display for Phi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Phi::True => write!(f, "true"),
            Phi::False => write!(f, "false"),
            Phi::Proposition(id) => write!(f, "{}", id),
            Phi::Not(formula) => write!(f, "!({})", formula),
            Phi::Or(left, right) => write!(f, "({} | {})", left, right),
            Phi::And(left, right) => write!(f, "({} & {})", left, right),
            Phi::DespiteNext { players, formula } => write!(
                f,
                "[[{}]] X {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
            Phi::EnforceNext { players, formula } => write!(
                f,
                "<<{}>> X {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
            Phi::DespiteUntil {
                players,
                pre,
                until,
            } => write!(
                f,
                "[[{}]] ({} U {})",
                players.iter().join_with(",").to_string(),
                pre,
                until
            ),
            Phi::EnforceUntil {
                players,
                pre,
                until,
            } => write!(
                f,
                "<<{}>> ({} U {})",
                players.iter().join_with(",").to_string(),
                pre,
                until
            ),
            Phi::DespiteEventually { players, formula } => write!(
                f,
                "[[{}]] F {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
            Phi::EnforceEventually { players, formula } => write!(
                f,
                "<<{}>> F {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
            Phi::DespiteInvariant { players, formula } => write!(
                f,
                "[[{}]] G {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
            Phi::EnforceInvariant { players, formula } => write!(
                f,
                "<<{}>> G {}",
                players.iter().join_with(",").to_string(),
                formula
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::atl::formula::Phi::*;
    use std::sync::Arc;

    #[test]
    fn test_display_01() {
        let formula = EnforceUntil {
            players: vec![0, 1],
            pre: Arc::new(Or {
                0: Arc::new(Proposition(1)),
                1: Arc::new(Not(Arc::new(Proposition(2)))),
            }),
            until: Arc::new(False),
        };
        assert_eq!("<<0,1>> ((1 | !(2)) U false)", format!("{}", formula));
    }
}
