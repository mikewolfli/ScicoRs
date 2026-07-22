//! Event queue and trigger system.
//!
//! Provides a time-sorted event queue for managing simulation events,
//! including time events, zero-crossing events, external triggers,
//! and conditional triggers.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::core::error::SimError;
use crate::core::types::{SignalValue, Time};

/// Type of simulation event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Scheduled time event.
    TimeEvent,
    /// Zero-crossing detection event.
    ZeroCrossing,
    /// External trigger event.
    External,
    /// Condition-based trigger event.
    Condition,
}

/// A single simulation event with timing and data.
#[derive(Debug, Clone)]
pub struct Event {
    /// Unique identifier for this event.
    pub id: String,
    /// Scheduled trigger time.
    pub time: Time,
    /// Type of event.
    pub event_type: EventType,
    /// Associated data payload.
    pub data: SignalValue,
    /// Priority (lower = higher priority, for same-time events).
    pub priority: u32,
}

impl Event {
    /// Create a new event.
    pub fn new(id: &str, time: Time, event_type: EventType, data: SignalValue) -> Self {
        Self {
            id: id.to_string(),
            time,
            event_type,
            data,
            priority: 0,
        }
    }

    /// Create a new event with explicit priority.
    pub fn with_priority(
        id: &str,
        time: Time,
        event_type: EventType,
        data: SignalValue,
        priority: u32,
    ) -> Self {
        Self {
            id: id.to_string(),
            time,
            event_type,
            data,
            priority,
        }
    }
}

/// Order events by time (earliest first), then by priority (lowest first).
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap, so we reverse for min-heap behavior
        other
            .time
            .partial_cmp(&self.time)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.priority.cmp(&self.priority))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        (self.time - other.time).abs() < 1e-15 && self.id == other.id
    }
}

impl Eq for Event {}

/// A time-sorted event queue using a binary heap.
///
/// Events are ordered by trigger time (earliest first) and
/// within the same time, by priority (lower = higher priority).
#[derive(Debug, Clone)]
pub struct EventQueue {
    /// The binary heap storing events (min-heap by time).
    heap: BinaryHeap<Event>,
    /// Number of events processed so far.
    pub processed: u64,
    /// Number of events currently pending.
    pub pending: u64,
    /// Maximum capacity.
    capacity: usize,
}

impl EventQueue {
    /// Create a new empty event queue.
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            processed: 0,
            pending: 0,
            capacity: usize::MAX,
        }
    }

    /// Create an event queue with a maximum capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(capacity.min(1024)),
            processed: 0,
            pending: 0,
            capacity,
        }
    }

    /// Push an event into the queue.
    ///
    /// Returns `Err` if the queue is at capacity.
    pub fn push(&mut self, event: Event) -> Result<(), SimError> {
        if self.pending >= self.capacity as u64 {
            return Err(SimError::runtime("event queue at capacity"));
        }
        self.pending += 1;
        self.heap.push(event);
        Ok(())
    }

    /// Pop the next event (earliest time, highest priority).
    ///
    /// Returns `None` if the queue is empty.
    pub fn pop(&mut self) -> Option<Event> {
        let event = self.heap.pop();
        if event.is_some() {
            self.pending = self.pending.saturating_sub(1);
            self.processed += 1;
        }
        event
    }

    /// Peek at the next event without removing it.
    pub fn peek(&self) -> Option<&Event> {
        self.heap.peek()
    }

    /// Return the time of the next event, if any.
    pub fn next_time(&self) -> Option<Time> {
        self.heap.peek().map(|e| e.time)
    }

    /// Return the number of pending events.
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Clear all pending events.
    pub fn clear(&mut self) {
        self.heap.clear();
        self.pending = 0;
    }

    /// Drain all events with time <= the given time.
    ///
    /// Returns events in chronological order.
    pub fn drain_up_to(&mut self, time: Time) -> Vec<Event> {
        let mut events = Vec::new();
        while let Some(event) = self.peek() {
            if event.time <= time + 1e-15 {
                events.push(self.pop().unwrap());
            } else {
                break;
            }
        }
        events
    }

    /// Remove all events matching a predicate.
    pub fn remove_where<F>(&mut self, mut pred: F)
    where
        F: FnMut(&Event) -> bool,
    {
        let remaining = self.heap.drain().filter(|e| !pred(e)).collect();
        self.heap = remaining;
        self.pending = self.heap.len() as u64;
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
// 1. ZeroCrossingDetector
// ──────────────────────────────────────────────

use crate::core::types::Scalar;
use std::collections::HashMap;

/// Detects zero-crossing events by tracking previous and current signal values
/// for (block_id, port) pairs and comparing against a configurable threshold.
#[derive(Debug, Clone)]
pub struct ZeroCrossingDetector {
    /// Previous-step values keyed by (block_id, port).
    pub prev: HashMap<(String, String), Scalar>,
    /// Current-step values keyed by (block_id, port).
    pub curr: HashMap<(String, String), Scalar>,
    /// Threshold for crossing detection (sign change is detected when
    /// the value crosses within this band around zero).
    pub threshold: Scalar,
}

impl ZeroCrossingDetector {
    /// Create a new detector with the given threshold.
    pub fn new(threshold: Scalar) -> Self {
        Self {
            prev: HashMap::new(),
            curr: HashMap::new(),
            threshold,
        }
    }

    /// Record a signal value for the current time step.
    pub fn update(&mut self, block_id: &str, port: &str, value: Scalar) {
        self.curr
            .insert((block_id.to_string(), port.to_string()), value);
    }

    /// Advance the detector: move current values to previous and clear current.
    pub fn advance(&mut self) {
        std::mem::swap(&mut self.prev, &mut self.curr);
        self.curr.clear();
    }

    /// Returns `true` if the signal crossed from <= -threshold to > +threshold.
    pub fn detect_rising(&self, block_id: &str, port: &str) -> bool {
        let key = (block_id.to_string(), port.to_string());
        match (self.prev.get(&key), self.curr.get(&key)) {
            (Some(&prev), Some(&curr)) => prev <= self.threshold && curr > self.threshold,
            _ => false,
        }
    }

    /// Returns `true` if the signal crossed from >= +threshold to < -threshold.
    pub fn detect_falling(&self, block_id: &str, port: &str) -> bool {
        let key = (block_id.to_string(), port.to_string());
        match (self.prev.get(&key), self.curr.get(&key)) {
            (Some(&prev), Some(&curr)) => prev >= -self.threshold && curr < -self.threshold,
            _ => false,
        }
    }

    /// Returns `true` if either a rising or falling zero crossing was detected.
    pub fn detect_any(&self, block_id: &str, port: &str) -> bool {
        self.detect_rising(block_id, port) || self.detect_falling(block_id, port)
    }

    /// Clear all stored values.
    pub fn clear(&mut self) {
        self.prev.clear();
        self.curr.clear();
    }
}

// ──────────────────────────────────────────────
// 2. TriggerCondition and EdgeType
// ──────────────────────────────────────────────

/// The edge direction for zero-crossing detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    /// Signal crosses from negative/zero to positive.
    Rising,
    /// Signal crosses from positive/zero to negative.
    Falling,
    /// Either direction.
    Both,
}

/// Describes a condition that can trigger an event.
#[derive(Debug, Clone)]
pub enum TriggerCondition {
    /// Fires at a specific simulation time.
    TimeTrigger(Time),
    /// Fires when a zero crossing is detected on a block port.
    ZeroCrossing {
        block: String,
        port: String,
        edge: EdgeType,
    },
    /// Fires when an external trigger arrives from the named source.
    External { source: String },
    /// Fires when a boolean expression (stored as a string for future evaluation)
    /// becomes true.
    Conditional { condition_expr: String },
    /// Fires on a hardware interrupt line.
    Interrupt { irq: u32 },
}

impl TriggerCondition {
    /// Return a human-readable description of this trigger condition.
    pub fn description(&self) -> String {
        match self {
            TriggerCondition::TimeTrigger(t) => format!("time trigger at {}", t),
            TriggerCondition::ZeroCrossing { block, port, edge } => {
                let edge_str = match edge {
                    EdgeType::Rising => "rising",
                    EdgeType::Falling => "falling",
                    EdgeType::Both => "any",
                };
                format!("zero-crossing {} on {}.{}", edge_str, block, port)
            }
            TriggerCondition::External { source } => {
                format!("external trigger from {}", source)
            }
            TriggerCondition::Conditional { condition_expr } => {
                format!("conditional trigger: {}", condition_expr)
            }
            TriggerCondition::Interrupt { irq } => format!("IRQ {}", irq),
        }
    }
}

// ──────────────────────────────────────────────
// 3. EventStatistics
// ──────────────────────────────────────────────

/// Collects statistics about event processing.
#[derive(Debug, Clone)]
pub struct EventStatistics {
    /// Total number of events recorded.
    pub total_events: u64,
    /// Count of events broken down by type.
    pub events_by_type: HashMap<EventType, u64>,
    /// Largest queue size observed.
    pub peak_queue_size: u64,
    /// Time of the most recently recorded event.
    pub last_event_time: Option<Time>,
}

impl EventStatistics {
    /// Create a new, empty statistics collector.
    pub fn new() -> Self {
        Self {
            total_events: 0,
            events_by_type: HashMap::new(),
            peak_queue_size: 0,
            last_event_time: None,
        }
    }

    /// Record an event's metadata into the statistics.
    pub fn record(&mut self, event: &Event) {
        self.total_events += 1;
        *self.events_by_type.entry(event.event_type).or_insert(0) += 1;
        self.last_event_time = Some(event.time);
    }

    /// Update the peak queue size if `size` is larger than the current peak.
    pub fn record_queue_size(&mut self, size: u64) {
        if size > self.peak_queue_size {
            self.peak_queue_size = size;
        }
    }

    /// Reset all statistics to their initial state.
    pub fn reset(&mut self) {
        self.total_events = 0;
        self.events_by_type.clear();
        self.peak_queue_size = 0;
        self.last_event_time = None;
    }
}

impl Default for EventStatistics {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
// 4. EventTriggerManager
// ──────────────────────────────────────────────

/// Top-level manager that ties together the event queue, zero-crossing detector,
/// statistics, and a list of registered trigger conditions.
#[derive(Debug, Clone)]
pub struct EventTriggerManager {
    /// The underlying event queue.
    pub queue: EventQueue,
    /// Zero-crossing detector for signal edge detection.
    pub detector: ZeroCrossingDetector,
    /// Event processing statistics.
    pub statistics: EventStatistics,
    /// Registered triggers paired with their event ID prefix.
    pub triggers: Vec<(TriggerCondition, String)>,
}

impl EventTriggerManager {
    /// Create a new empty manager with default components.
    pub fn new() -> Self {
        Self {
            queue: EventQueue::new(),
            detector: ZeroCrossingDetector::new(1e-12),
            statistics: EventStatistics::new(),
            triggers: Vec::new(),
        }
    }

    /// Register a new trigger condition with an event ID prefix.
    pub fn register_trigger(&mut self, condition: TriggerCondition, event_id_prefix: &str) {
        self.triggers.push((condition, event_id_prefix.to_string()));
    }

    /// Process all time triggers whose scheduled time ≤ `current_time`.
    ///
    /// Matching triggers are removed after firing and the count of fired events
    /// is returned.
    pub fn process_time_triggers(&mut self, current_time: Time) -> Result<u64, SimError> {
        let mut count = 0u64;
        // Collect indices of matching time triggers in reverse order so
        // swap_remove does not invalidate subsequent indices.
        let mut indices: Vec<usize> = self
            .triggers
            .iter()
            .enumerate()
            .filter_map(|(i, (cond, _))| {
                if let TriggerCondition::TimeTrigger(t) = cond {
                    if *t <= current_time + 1e-15 {
                        Some(i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        indices.reverse();

        for idx in indices {
            let (cond, prefix) = self.triggers.swap_remove(idx);
            if let TriggerCondition::TimeTrigger(t) = cond {
                self.fire_event(EventType::TimeEvent, SignalValue::Scalar(t), &prefix)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Check all registered zero-crossing triggers and fire events for
    /// those whose edge has been detected. Returns the number of fired events.
    pub fn process_zero_crossings(&mut self) -> u64 {
        let mut count = 0u64;
        let snapshot: Vec<(TriggerCondition, String)> = self.triggers.clone();
        for (cond, prefix) in &snapshot {
            if let TriggerCondition::ZeroCrossing { block, port, edge } = cond {
                let detected = match edge {
                    EdgeType::Rising => self.detector.detect_rising(block, port),
                    EdgeType::Falling => self.detector.detect_falling(block, port),
                    EdgeType::Both => self.detector.detect_any(block, port),
                };
                if detected {
                    // Ignore queue-full errors during zero-crossing processing;
                    // a full queue is a separate system condition.
                    let _ =
                        self.fire_event(EventType::ZeroCrossing, SignalValue::Scalar(0.0), prefix);
                    count += 1;
                }
            }
        }
        count
    }

    /// Process an external trigger from a named source, firing events for all
    /// matching `External` triggers.
    pub fn process_external_trigger(
        &mut self,
        source: &str,
        data: SignalValue,
    ) -> Result<(), SimError> {
        let snapshot: Vec<(TriggerCondition, String)> = self.triggers.clone();
        for (cond, prefix) in &snapshot {
            if let TriggerCondition::External { source: s } = cond
                && s == source
            {
                self.fire_event(EventType::External, data.clone(), prefix)?;
            }
        }
        Ok(())
    }

    /// Create an event with the given type, payload, and ID prefix, then push it
    /// onto the queue. The event time is taken from the queue's next pending time,
    /// or 0.0 if the queue is empty.
    pub fn fire_event(
        &mut self,
        event_type: EventType,
        data: SignalValue,
        id_prefix: &str,
    ) -> Result<(), SimError> {
        let id = format!("{}-{}", id_prefix, self.statistics.total_events);
        let time = self.queue.next_time().unwrap_or(0.0);
        let event = Event::new(&id, time, event_type, data);
        self.statistics.record(&event);
        self.queue.push(event)?;
        self.statistics.record_queue_size(self.queue.pending);
        Ok(())
    }

    /// Advance the zero-crossing detector to the next time step.
    pub fn advance_detector(&mut self) {
        self.detector.advance();
    }

    /// Clear the queue, detector, statistics, and all registered triggers.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.detector.clear();
        self.statistics.reset();
        self.triggers.clear();
    }
}

impl Default for EventTriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────
// 5. Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::SignalValue;

    #[test]
    fn test_event_queue_create() {
        let queue = EventQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_event_push_pop() {
        let mut queue = EventQueue::new();
        let event = Event::new("e1", 1.0, EventType::TimeEvent, SignalValue::Scalar(42.0));
        queue.push(event).unwrap();

        assert!(!queue.is_empty());
        assert_eq!(queue.pending, 1);

        let popped = queue.pop().unwrap();
        assert_eq!(popped.id, "e1");
        assert!((popped.time - 1.0).abs() < 1e-15);
        assert_eq!(queue.processed, 1);
    }

    #[test]
    fn test_event_time_ordering() {
        let mut queue = EventQueue::new();

        queue
            .push(Event::new(
                "e1",
                3.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        queue
            .push(Event::new(
                "e2",
                1.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        queue
            .push(Event::new(
                "e3",
                2.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();

        // Events should come out in time order
        assert_eq!(queue.pop().unwrap().id, "e2");
        assert_eq!(queue.pop().unwrap().id, "e3");
        assert_eq!(queue.pop().unwrap().id, "e1");
    }

    #[test]
    fn test_drain_up_to() {
        let mut queue = EventQueue::new();
        queue
            .push(Event::new(
                "e1",
                1.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        queue
            .push(Event::new(
                "e2",
                2.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        queue
            .push(Event::new(
                "e3",
                3.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();

        let drained = queue.drain_up_to(2.0);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, "e1");
        assert_eq!(drained[1].id, "e2");
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_event_capacity() {
        let mut queue = EventQueue::with_capacity(2);
        queue
            .push(Event::new(
                "e1",
                1.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        queue
            .push(Event::new(
                "e2",
                2.0,
                EventType::TimeEvent,
                SignalValue::None,
            ))
            .unwrap();
        let result = queue.push(Event::new(
            "e3",
            3.0,
            EventType::TimeEvent,
            SignalValue::None,
        ));
        assert!(result.is_err());
    }

    // ── ZeroCrossingDetector tests ──────────────────────────────────────

    #[test]
    fn test_zcd_rising() {
        let mut zcd = ZeroCrossingDetector::new(1e-12);
        zcd.update("b1", "out", -1.0);
        zcd.advance();
        zcd.update("b1", "out", 2.0);
        assert!(zcd.detect_rising("b1", "out"));
        assert!(!zcd.detect_falling("b1", "out"));
        assert!(zcd.detect_any("b1", "out"));
    }

    #[test]
    fn test_zcd_falling() {
        let mut zcd = ZeroCrossingDetector::new(1e-12);
        zcd.update("b1", "out", 3.0);
        zcd.advance();
        zcd.update("b1", "out", -5.0);
        assert!(!zcd.detect_rising("b1", "out"));
        assert!(zcd.detect_falling("b1", "out"));
        assert!(zcd.detect_any("b1", "out"));
    }

    #[test]
    fn test_zcd_no_crossing() {
        let mut zcd = ZeroCrossingDetector::new(1e-12);
        zcd.update("b1", "out", -1.0);
        zcd.advance();
        zcd.update("b1", "out", -2.0);
        assert!(!zcd.detect_any("b1", "out"));
    }

    #[test]
    fn test_zcd_missing_signal() {
        let zcd = ZeroCrossingDetector::new(1e-12);
        // No data recorded for this key at all
        assert!(!zcd.detect_any("unknown", "x"));
    }

    #[test]
    fn test_zcd_advance_and_clear() {
        let mut zcd = ZeroCrossingDetector::new(1e-12);
        zcd.update("b1", "out", -1.0);
        zcd.advance();
        // After advance, prev has -1.0, curr is empty
        zcd.update("b1", "out", 1.0);
        assert!(zcd.detect_rising("b1", "out"));

        zcd.clear();
        assert!(!zcd.detect_any("b1", "out"));
        assert!(zcd.prev.is_empty());
        assert!(zcd.curr.is_empty());
    }

    // ── TriggerCondition tests ──────────────────────────────────────────

    #[test]
    fn test_trigger_condition_descriptions() {
        let t1 = TriggerCondition::TimeTrigger(42.0);
        assert_eq!(t1.description(), "time trigger at 42");

        let t2 = TriggerCondition::ZeroCrossing {
            block: "sensor".into(),
            port: "out".into(),
            edge: EdgeType::Rising,
        };
        assert_eq!(t2.description(), "zero-crossing rising on sensor.out");

        let t3 = TriggerCondition::ZeroCrossing {
            block: "sensor".into(),
            port: "out".into(),
            edge: EdgeType::Falling,
        };
        assert_eq!(t3.description(), "zero-crossing falling on sensor.out");

        let t4 = TriggerCondition::ZeroCrossing {
            block: "sensor".into(),
            port: "out".into(),
            edge: EdgeType::Both,
        };
        assert_eq!(t4.description(), "zero-crossing any on sensor.out");

        let t5 = TriggerCondition::External {
            source: "uart".into(),
        };
        assert_eq!(t5.description(), "external trigger from uart");

        let t6 = TriggerCondition::Conditional {
            condition_expr: "x > 0.0".into(),
        };
        assert_eq!(t6.description(), "conditional trigger: x > 0.0");

        let t7 = TriggerCondition::Interrupt { irq: 7 };
        assert_eq!(t7.description(), "IRQ 7");
    }

    // ── EventStatistics tests ───────────────────────────────────────────

    #[test]
    fn test_event_statistics_record() {
        let mut stats = EventStatistics::new();
        assert_eq!(stats.total_events, 0);
        assert!(stats.last_event_time.is_none());

        let e1 = Event::new("a", 1.0, EventType::TimeEvent, SignalValue::Scalar(10.0));
        stats.record(&e1);
        assert_eq!(stats.total_events, 1);
        assert_eq!(*stats.events_by_type.get(&EventType::TimeEvent).unwrap(), 1);
        assert!((stats.last_event_time.unwrap() - 1.0).abs() < 1e-15);

        let e2 = Event::new("b", 2.0, EventType::ZeroCrossing, SignalValue::Scalar(0.0));
        stats.record(&e2);
        assert_eq!(stats.total_events, 2);
        assert_eq!(*stats.events_by_type.get(&EventType::TimeEvent).unwrap(), 1);
        assert_eq!(
            *stats.events_by_type.get(&EventType::ZeroCrossing).unwrap(),
            1
        );
        assert!((stats.last_event_time.unwrap() - 2.0).abs() < 1e-15);
    }

    #[test]
    fn test_event_statistics_record_queue_size() {
        let mut stats = EventStatistics::new();
        assert_eq!(stats.peak_queue_size, 0);

        stats.record_queue_size(5);
        assert_eq!(stats.peak_queue_size, 5);

        // Smaller value should NOT update the peak
        stats.record_queue_size(3);
        assert_eq!(stats.peak_queue_size, 5);

        // Larger value should update the peak
        stats.record_queue_size(10);
        assert_eq!(stats.peak_queue_size, 10);
    }

    #[test]
    fn test_event_statistics_reset() {
        let mut stats = EventStatistics::new();
        let e = Event::new("x", 1.0, EventType::External, SignalValue::Boolean(true));
        stats.record(&e);
        stats.record_queue_size(7);
        assert_eq!(stats.total_events, 1);

        stats.reset();
        assert_eq!(stats.total_events, 0);
        assert!(stats.events_by_type.is_empty());
        assert_eq!(stats.peak_queue_size, 0);
        assert!(stats.last_event_time.is_none());
    }

    // ── EventTriggerManager tests ──────────────────────────────────────

    #[test]
    fn test_etm_register_and_fire_event() {
        let mut mgr = EventTriggerManager::new();
        mgr.register_trigger(TriggerCondition::TimeTrigger(5.0), "timer_a");
        mgr.register_trigger(
            TriggerCondition::External {
                source: "ext1".into(),
            },
            "ext_a",
        );

        assert_eq!(mgr.triggers.len(), 2);

        // fire_event directly
        mgr.fire_event(EventType::Condition, SignalValue::Scalar(99.0), "manual")
            .unwrap();
        assert_eq!(mgr.queue.len(), 1);
        assert_eq!(mgr.statistics.total_events, 1);

        let ev = mgr.queue.peek().unwrap();
        assert_eq!(ev.event_type, EventType::Condition);
        assert_eq!(ev.data, SignalValue::Scalar(99.0));
    }

    #[test]
    fn test_etm_process_time_triggers() {
        let mut mgr = EventTriggerManager::new();
        mgr.register_trigger(TriggerCondition::TimeTrigger(1.0), "early");
        mgr.register_trigger(TriggerCondition::TimeTrigger(3.0), "late");
        mgr.register_trigger(
            TriggerCondition::External {
                source: "ignored".into(),
            },
            "ext",
        );

        // Process at time 2.0 — only "early" should fire
        let count = mgr.process_time_triggers(2.0).unwrap();
        assert_eq!(count, 1);
        assert_eq!(mgr.queue.len(), 1);
        assert_eq!(mgr.statistics.total_events, 1);

        let ev = mgr.queue.peek().unwrap();
        assert_eq!(ev.event_type, EventType::TimeEvent);
        assert!(ev.id.starts_with("early"));

        // Also the TimeTrigger for "late" should have been removed from triggers
        let remaining_time_triggers: usize = mgr
            .triggers
            .iter()
            .filter(|(c, _)| matches!(c, TriggerCondition::TimeTrigger(_)))
            .count();
        assert_eq!(remaining_time_triggers, 1);
    }

    #[test]
    fn test_etm_process_zero_crossings() {
        let mut mgr = EventTriggerManager::new();
        mgr.register_trigger(
            TriggerCondition::ZeroCrossing {
                block: "b1".into(),
                port: "out".into(),
                edge: EdgeType::Rising,
            },
            "zc",
        );

        // Set up a rising crossing
        mgr.detector.update("b1", "out", -1.0);
        mgr.detector.advance();
        mgr.detector.update("b1", "out", 2.0);

        let count = mgr.process_zero_crossings();
        assert_eq!(count, 1);
        assert_eq!(mgr.queue.len(), 1);

        let ev = mgr.queue.peek().unwrap();
        assert_eq!(ev.event_type, EventType::ZeroCrossing);
        assert!(ev.id.starts_with("zc"));
    }

    #[test]
    fn test_etm_clear() {
        let mut mgr = EventTriggerManager::new();
        mgr.register_trigger(TriggerCondition::Interrupt { irq: 1 }, "irq");
        mgr.fire_event(EventType::External, SignalValue::Scalar(0.0), "init")
            .unwrap();

        mgr.clear();
        assert!(mgr.queue.is_empty());
        assert!(mgr.detector.prev.is_empty());
        assert!(mgr.detector.curr.is_empty());
        assert_eq!(mgr.statistics.total_events, 0);
        assert!(mgr.triggers.is_empty());
    }

    #[test]
    fn test_etm_advance_detector() {
        let mut mgr = EventTriggerManager::new();
        mgr.detector.update("b1", "out", -1.0);
        mgr.advance_detector();
        assert!((mgr.detector.prev.get(&("b1".into(), "out".into())).unwrap() + 1.0).abs() < 1e-15);
        assert!(mgr.detector.curr.is_empty());
    }
}
