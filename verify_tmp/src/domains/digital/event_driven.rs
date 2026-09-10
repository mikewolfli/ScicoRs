//! Event-driven digital simulation.
use crate::core::types::Scalar;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
#[derive(Debug, Clone)]
pub struct EventItem { pub time: Scalar, pub signal: String, pub value: Scalar }
impl Eq for EventItem {}
impl PartialEq for EventItem { fn eq(&self, other: &Self) -> bool { (self.time - other.time).abs() < 1e-30 } }
impl Ord for EventItem { fn cmp(&self, other: &Self) -> Ordering { other.time.partial_cmp(&self.time).unwrap_or(Ordering::Equal) } }
impl PartialOrd for EventItem { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }
#[derive(Debug, Clone)]
pub struct EventDrivenSimulator {
    pub events: BinaryHeap<EventItem>, pub time: Scalar, pub signal_values: HashMap<String, Scalar>,
}
impl EventDrivenSimulator {
    pub fn new() -> Self { Self { events: BinaryHeap::new(), time: 0.0, signal_values: HashMap::new() } }
    pub fn schedule(&mut self, delay: Scalar, signal: String, value: Scalar) { self.events.push(EventItem { time: self.time + delay, signal, value }); }
    pub fn run(&mut self, t_end: Scalar) -> Result<(), String> {
        while let Some(event) = self.events.peek() {
            if event.time > t_end { break; }
            let event = self.events.pop().unwrap();
            self.time = event.time;
            self.signal_values.insert(event.signal.clone(), event.value);
        }
        Ok(())
    }
    pub fn vcd_dump(&self, filepath: &str) -> Result<(), String> {
        let mut s = String::from("$date\n\tSimulation\n$end\n$timescale 1 ps $end\n");
        for (sig, &val) in &self.signal_values {
            s.push_str(&format!("$var real 1 {} {} $end\n", sig.chars().next().unwrap_or('x'), sig));
            s.push_str(&format!("#{}\n{:.3}\n", (self.time * 1e12) as u64, val));
        }
        std::fs::write(filepath, s).map_err(|e| format!("Write error: {}", e))
    }
}
impl Default for EventDrivenSimulator { fn default() -> Self { Self::new() } }
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_sim_new() { let s = EventDrivenSimulator::new(); assert!((s.time - 0.0).abs() < 1e-10); }
    #[test] fn test_schedule_and_run() {
        let mut sim = EventDrivenSimulator::new();
        sim.schedule(1e-9, "clk".to_string(), 1.0);
        sim.run(10e-9).unwrap();
        assert!((sim.signal_values.get("clk").unwrap() - 1.0).abs() < 1e-10);
    }
}
