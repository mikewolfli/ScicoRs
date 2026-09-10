//! Multi-rate task scheduling with clock domain isolation.
//!
//! Supports multiple clock domains operating at different rates,
//! phase offsets, and synchronization at common multiples.

use crate::core::block::BlockId;
use crate::core::diagram::Diagram;
use crate::core::types::Scalar;
use std::collections::HashMap;

/// A clock domain defining a specific rate and phase offset.
#[derive(Debug, Clone)]
pub struct ClockDomain {
    /// Unique identifier for this clock domain.
    pub id: String,
    /// Base rate (step size in seconds) for this domain.
    pub base_rate: Scalar,
    /// Phase offset (0..base_rate) in seconds.
    pub offset: Scalar,
}

impl ClockDomain {
    /// Create a new clock domain with the given rate and offset.
    pub fn new(id: &str, base_rate: Scalar, offset: Scalar) -> Self {
        Self {
            id: id.to_string(),
            base_rate: base_rate.max(1e-15),
            offset: offset.clamp(0.0, base_rate),
        }
    }

    /// Compute the next trigger time for this domain at or after current_time.
    pub fn next_trigger(&self, current_time: Scalar) -> Scalar {
        let period = self.base_rate;
        if period <= 0.0 {
            return current_time;
        }
        let t0 = self.offset;
        if current_time <= t0 {
            return t0;
        }
        let n = ((current_time - t0) / period).ceil();
        t0 + n * period
    }

    /// Returns true if this domain should be active at the given time.
    pub fn is_active_at(&self, time: Scalar, epsilon: Scalar) -> bool {
        if self.base_rate <= 0.0 {
            return true;
        }
        let phase = (time - self.offset) / self.base_rate;
        let remainder = phase - phase.floor();
        remainder < epsilon || (1.0 - remainder) < epsilon
    }
}

/// Multi-rate scheduler managing multiple clock domains.
#[derive(Debug, Clone)]
pub struct MultiRateScheduler {
    /// All clock domains.
    pub domains: Vec<ClockDomain>,
    /// Block to domain assignment: block_id -> domain_id.
    pub block_assignments: HashMap<BlockId, String>,
    /// Domain rates for quick lookup: domain_id -> base_rate.
    pub domain_rates: HashMap<String, Scalar>,
}

impl MultiRateScheduler {
    /// Create a new empty multi-rate scheduler.
    pub fn new() -> Self {
        Self {
            domains: Vec::new(),
            block_assignments: HashMap::new(),
            domain_rates: HashMap::new(),
        }
    }

    /// Add a clock domain.
    pub fn add_domain(&mut self, domain: ClockDomain) {
        self.domain_rates
            .insert(domain.id.clone(), domain.base_rate);
        self.domains.push(domain);
    }

    /// Assign a block to a clock domain.
    pub fn assign_block(&mut self, block_id: &str, domain_id: &str) {
        self.block_assignments
            .insert(block_id.to_string(), domain_id.to_string());
    }

    /// Get the domain for a block, if assigned.
    pub fn block_domain(&self, block_id: &str) -> Option<&ClockDomain> {
        let domain_id = self.block_assignments.get(block_id)?;
        self.domains.iter().find(|d| &d.id == domain_id)
    }

    /// Find all domains that should trigger at the given time.
    pub fn active_domains_at(&self, time: Scalar, epsilon: Scalar) -> Vec<&ClockDomain> {
        self.domains
            .iter()
            .filter(|d| d.is_active_at(time, epsilon))
            .collect()
    }

    /// Find synchronization points (times when all domains align).
    pub fn sync_points(&self, start: Scalar, end: Scalar, max_points: usize) -> Vec<Scalar> {
        if self.domains.is_empty() {
            return Vec::new();
        }
        let mut lcm_period = self.domains[0].base_rate;
        for domain in &self.domains[1..] {
            lcm_period = lcm(lcm_period, domain.base_rate);
        }

        let mut points = Vec::new();
        let mut t = self.domains[0].next_trigger(start);
        while t <= end && points.len() < max_points {
            let all_active = self.domains.iter().all(|d| d.is_active_at(t, 1e-12));
            if all_active {
                points.push(t);
            }
            if lcm_period > 1e10 || lcm_period <= 0.0 {
                let min_rate = self
                    .domains
                    .iter()
                    .map(|d| d.base_rate)
                    .fold(f64::INFINITY, |a, b| a.min(b));
                t += min_rate;
            } else {
                t += lcm_period;
            }
        }
        points
    }
}

impl Default for MultiRateScheduler {
    fn default() -> Self {
        Self::new()
    }
}

fn lcm(a: Scalar, b: Scalar) -> Scalar {
    if a <= 0.0 || b <= 0.0 {
        return a.max(b);
    }
    let gcd_val = gcd(a, b);
    a * b / gcd_val
}

fn gcd(a: Scalar, b: Scalar) -> Scalar {
    let mut a = a.abs();
    let mut b = b.abs();
    let eps = 1e-12;
    while b > eps {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Build a MultiRateScheduler from a diagram by analyzing block rates.
pub fn build_multirate_schedule(diagram: &Diagram, discrete_step: Scalar) -> MultiRateScheduler {
    let mut scheduler = MultiRateScheduler::new();
    scheduler.add_domain(ClockDomain::new("continuous", discrete_step, 0.0));
    scheduler.add_domain(ClockDomain::new("base", discrete_step, 0.0));

    for (id, block) in diagram.blocks() {
        if block.state_declaration().continuous_count() > 0 {
            scheduler.assign_block(id, "continuous");
        } else {
            scheduler.assign_block(id, "base");
        }
    }
    scheduler
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_domain_creation() {
        let domain = ClockDomain::new("fast", 0.01, 0.0);
        assert_eq!(domain.id, "fast");
        assert!((domain.base_rate - 0.01).abs() < 1e-15);
    }

    #[test]
    fn test_next_trigger() {
        let domain = ClockDomain::new("rate_0_1", 0.1, 0.0);
        assert!((domain.next_trigger(0.0) - 0.0).abs() < 1e-12);
        assert!((domain.next_trigger(0.05) - 0.1).abs() < 1e-12);
        assert!((domain.next_trigger(0.1) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn test_is_active_at() {
        let domain = ClockDomain::new("rate_1", 1.0, 0.0);
        assert!(domain.is_active_at(0.0, 1e-12));
        assert!(!domain.is_active_at(0.5, 1e-12));
        assert!(domain.is_active_at(1.0, 1e-12));
    }

    #[test]
    fn test_multirate_scheduler_create() {
        let mut scheduler = MultiRateScheduler::new();
        scheduler.add_domain(ClockDomain::new("fast", 0.01, 0.0));
        scheduler.add_domain(ClockDomain::new("slow", 0.1, 0.0));
        scheduler.assign_block("B1", "fast");
        scheduler.assign_block("B2", "slow");

        assert!(scheduler.block_domain("B1").is_some());
        assert!(scheduler.block_domain("B2").is_some());
        assert!(scheduler.block_domain("B3").is_none());
    }

    #[test]
    fn test_sync_points() {
        let mut scheduler = MultiRateScheduler::new();
        scheduler.add_domain(ClockDomain::new("fast", 0.1, 0.0));
        scheduler.add_domain(ClockDomain::new("slow", 0.5, 0.0));

        let syncs = scheduler.sync_points(0.0, 2.0, 10);
        assert!(!syncs.is_empty());
        assert!((syncs[0] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_active_domains() {
        let mut scheduler = MultiRateScheduler::new();
        scheduler.add_domain(ClockDomain::new("fast", 0.1, 0.0));
        scheduler.add_domain(ClockDomain::new("slow", 0.5, 0.0));

        let active = scheduler.active_domains_at(0.0, 1e-12);
        assert_eq!(active.len(), 2);

        let active2 = scheduler.active_domains_at(0.1, 1e-12);
        assert_eq!(active2.len(), 1); // only fast at t=0.1
    }
}
