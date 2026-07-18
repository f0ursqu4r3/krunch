//! Deterministic consensus guard (PLAN §3h).
//!
//! The mediator is the final arbiter, but it cannot declare `CONSENSUS` against
//! the evidence. This module computes, purely over structured fields:
//!
//! 1. an **undirected agreement graph using only reciprocal `agree_with` edges**
//!    (A→B counts only if B→A also holds — one-sided claims are ignored),
//! 2. the largest connected component of that graph, which must cover
//!    `>= quorum_fraction` of surviving panelists, **and**
//! 3. mean survivor confidence `>= confidence_floor`.
//!
//! Stance *prose* is never compared — that would be non-deterministic.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::GuardThresholds;
use crate::ids::SeatId;
use crate::schema::Stance;

/// One surviving (non-abstained) panelist's validated stance.
///
/// `agree_with` is assumed already validated by [`crate::parse::parse_stance`]
/// (no self / unknown / duplicate refs).
#[derive(Debug, Clone)]
pub struct SurvivorStance {
    pub seat: SeatId,
    pub stance: Stance,
}

/// Result of evaluating the guard for a round.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardOutcome {
    /// Whether `CONSENSUS` may be accepted.
    pub consensus_ok: bool,
    /// Fraction of survivors covered by the largest reciprocal-agreement cluster.
    pub cluster_fraction: f64,
    /// Mean survivor confidence.
    pub mean_confidence: f64,
    /// The seats in the largest reciprocal-agreement cluster (sorted).
    pub largest_cluster: Vec<SeatId>,
    /// Number of survivors used as the denominator.
    pub survivor_count: usize,
}

/// Evaluate the consensus guard over the round's surviving stances.
pub fn evaluate_consensus(survivors: &[SurvivorStance], thresholds: GuardThresholds) -> GuardOutcome {
    let n = survivors.len();
    if n == 0 {
        return GuardOutcome {
            consensus_ok: false,
            cluster_fraction: 0.0,
            mean_confidence: 0.0,
            largest_cluster: Vec::new(),
            survivor_count: 0,
        };
    }

    let survivor_ids: BTreeSet<SeatId> = survivors.iter().map(|s| s.seat).collect();
    let claims: BTreeMap<SeatId, BTreeSet<SeatId>> = survivors
        .iter()
        .map(|s| (s.seat, s.stance.agree_with.iter().copied().collect()))
        .collect();

    // Build undirected adjacency from reciprocal edges among survivors only.
    let mut adj: BTreeMap<SeatId, BTreeSet<SeatId>> =
        survivor_ids.iter().map(|&s| (s, BTreeSet::new())).collect();
    let ids: Vec<SeatId> = survivor_ids.iter().copied().collect();
    for (i, &a) in ids.iter().enumerate() {
        for &b in &ids[i + 1..] {
            let a_to_b = claims.get(&a).map(|s| s.contains(&b)).unwrap_or(false);
            let b_to_a = claims.get(&b).map(|s| s.contains(&a)).unwrap_or(false);
            if a_to_b && b_to_a {
                adj.get_mut(&a).unwrap().insert(b);
                adj.get_mut(&b).unwrap().insert(a);
            }
        }
    }

    // Largest connected component via BFS over the reciprocal graph.
    let mut visited: BTreeSet<SeatId> = BTreeSet::new();
    let mut best: Vec<SeatId> = Vec::new();
    for &start in &ids {
        if visited.contains(&start) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = vec![start];
        visited.insert(start);
        while let Some(node) = queue.pop() {
            component.push(node);
            for &nbr in &adj[&node] {
                if visited.insert(nbr) {
                    queue.push(nbr);
                }
            }
        }
        if component.len() > best.len() {
            component.sort();
            best = component;
        }
    }

    let cluster_fraction = best.len() as f64 / n as f64;
    let mean_confidence =
        survivors.iter().map(|s| s.stance.confidence).sum::<f64>() / n as f64;

    let consensus_ok = cluster_fraction >= thresholds.quorum_fraction
        && mean_confidence >= thresholds.confidence_floor;

    GuardOutcome {
        consensus_ok,
        cluster_fraction,
        mean_confidence,
        largest_cluster: best,
        survivor_count: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stance(confidence: f64, agree: &[SeatId]) -> Stance {
        Stance {
            v: 1,
            stance: "s".into(),
            confidence,
            agree_with: agree.to_vec(),
            open_questions: vec![],
        }
    }

    fn survivor(seat: SeatId, confidence: f64, agree: &[SeatId]) -> SurvivorStance {
        SurvivorStance { seat, stance: stance(confidence, agree) }
    }

    #[test]
    fn unanimous_reciprocal_agreement_passes() {
        let (a, b, c) = (SeatId::new(), SeatId::new(), SeatId::new());
        let survivors = vec![
            survivor(a, 0.9, &[b, c]),
            survivor(b, 0.8, &[a, c]),
            survivor(c, 0.85, &[a, b]),
        ];
        let out = evaluate_consensus(&survivors, GuardThresholds::default());
        assert!(out.consensus_ok);
        assert_eq!(out.cluster_fraction, 1.0);
        assert_eq!(out.largest_cluster.len(), 3);
    }

    #[test]
    fn one_sided_claims_do_not_form_a_cluster() {
        // a claims agreement with everyone, but nobody reciprocates.
        let (a, b, c) = (SeatId::new(), SeatId::new(), SeatId::new());
        let survivors = vec![
            survivor(a, 0.9, &[b, c]),
            survivor(b, 0.9, &[]),
            survivor(c, 0.9, &[]),
        ];
        let out = evaluate_consensus(&survivors, GuardThresholds::default());
        // Largest reciprocal cluster is a single seat -> 1/3, below 2/3.
        assert_eq!(out.largest_cluster.len(), 1);
        assert!(!out.consensus_ok);
    }

    #[test]
    fn two_of_three_reciprocal_meets_two_thirds_quorum() {
        let (a, b, c) = (SeatId::new(), SeatId::new(), SeatId::new());
        // a<->b reciprocal, c isolated.
        let survivors = vec![
            survivor(a, 0.9, &[b]),
            survivor(b, 0.9, &[a]),
            survivor(c, 0.9, &[]),
        ];
        let out = evaluate_consensus(&survivors, GuardThresholds::default());
        assert_eq!(out.largest_cluster.len(), 2);
        assert!((out.cluster_fraction - 2.0 / 3.0).abs() < 1e-9);
        assert!(out.consensus_ok); // 2/3 >= 2/3 and confidence high
    }

    #[test]
    fn quorum_met_but_low_confidence_fails() {
        let (a, b, c) = (SeatId::new(), SeatId::new(), SeatId::new());
        let survivors = vec![
            survivor(a, 0.3, &[b, c]),
            survivor(b, 0.3, &[a, c]),
            survivor(c, 0.3, &[a, b]),
        ];
        let out = evaluate_consensus(&survivors, GuardThresholds::default());
        assert_eq!(out.cluster_fraction, 1.0);
        assert!(out.mean_confidence < 0.6);
        assert!(!out.consensus_ok);
    }

    #[test]
    fn two_disjoint_pairs_take_the_larger_cluster() {
        let (a, b, c, d) = (SeatId::new(), SeatId::new(), SeatId::new(), SeatId::new());
        // a<->b<->c chain (component of 3), d isolated.
        let survivors = vec![
            survivor(a, 0.9, &[b]),
            survivor(b, 0.9, &[a, c]),
            survivor(c, 0.9, &[b]),
            survivor(d, 0.9, &[]),
        ];
        let out = evaluate_consensus(&survivors, GuardThresholds::default());
        assert_eq!(out.largest_cluster.len(), 3);
        assert_eq!(out.cluster_fraction, 0.75);
        assert!(out.consensus_ok);
    }

    #[test]
    fn empty_survivors_never_consensus() {
        let out = evaluate_consensus(&[], GuardThresholds::default());
        assert!(!out.consensus_ok);
        assert_eq!(out.survivor_count, 0);
    }
}
