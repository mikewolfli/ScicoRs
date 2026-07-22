//! Event and Trigger System
//!
//! Manages time-based events, zero-crossing detection, external triggers,
//! and conditional event handling within the simulation.

use crate::core::types::{Scalar, SignalValue, Time};
use std::collections::BinaryHeap;

/// Priority of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    High = 0,
    Normal = 1,
    Low = 2,
}

/// Types of events in the simulation.
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    /// Scheduled at a specific simulation time.
    TimeEvent(Time),
    /// Triggered when a signal crosses zero.
    ZeroCrossing { block_id: String, port_id: String, direction: CrossingDirection },
    /// Triggered by an external source.
    External(String),
    /// Triggered when a condition is met.
    Conditional(String),
}

/// Direction of a zero crossing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossingDirection {
    /// Crossing from negative to positive (rising edge).
    Rising,
    /// Crossing from positive to negative (falling edge).
    Falling,
    /// Any crossing (either direction).
    Either,
}

/// An event in the simulation event queue.
#[derive(Debug, Clone)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub priority: EventPriority,
    pub payload: Option<SignalValue>,
}

impl Event {
    pub fn new_time(id: &str, time: Time) -> Self {
        Self {
            id: id.to_string(),
            event_type: EventType::TimeEvent(time),
            priority: EventPriority::Normal,
            payload: None,
        }
    }

    pub fn new_external(id: &str, source: &str) -> Self {
        Self {
            id: id.to_string(),
            event_type: EventType::External(source.to_string()),
            priority: EventPriority::Normal,
            payload: None,
        }
    }

    pub fn with_payload(mut self, payload: SignalValue) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Event {}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // BinaryHeap is a max-heap, so we invert for min-heap behavior
        let time_cmp = match (&self.event_type, &other.event_type) {
            (EventType::TimeEvent(t1), EventType::TimeEvent(t2)) => t2.partial_cmp(t1).unwrap_or(std::cmp::Ordering::Equal),
            (EventType::TimeEvent(_), _) => std::cmp::Ordering::Greater,
            (_, EventType::TimeEvent(_)) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        };
        time_cmp.then_with(|| self.priority.cmp(&other.priority))
    }
}

/// The event manager — maintains a priority queue of events.
#[derive(Debug)]
pub struct EventManager {
    events: BinaryHeap<Event>,
    processed_count: u64,
}

impl EventManager {
    pub fn new() -> Self {
        Self {
            events: BinaryHeap::new(),
            processed_count: 0,
        }
    }

    /// Schedule an event.
    pub fn schedule(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Pop the next event (earliest time, highest priority).
    pub fn dequeue(&mut self) -> Option<Event> {
        let event = self.events.pop()?;
        self.processed_count += 1;
        Some(event)
    }

    /// Peek at the next event without removing it.
    pub fn peek(&self) -> Option<&Event> {
        self.events.peek()
    }

    /// Number of pending events.
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }

    /// Number of events processed so far.
    pub fn processed_count(&self) -> u64 {
        self.processed_count
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Check if the event queue is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Detect zero crossings in a signal history.
    ///
    /// Returns the crossing direction if a zero crossing is detected.
    pub fn detect_zero_crossing(prev: Scalar, curr: Scalar, direction: CrossingDirection) -> Option<CrossingDirection> {
        match direction {
            CrossingDirection::Rising => {
                if prev <= 0.0 && curr > 0.0 { Some(CrossingDirection::Rising) } else { None }
            }
            CrossingDirection::Falling => {
                if prev >= 0.0 && curr < 0.0 { Some(CrossingDirection::Falling) } else { None }
            }
            CrossingDirection::Either => {
                if (prev <= 0.0 && curr > 0.0) || (prev >= 0.0 && curr < 0.0) {
                    Some(if curr > prev { CrossingDirection::Rising } else { CrossingDirection::Falling })
                } else {
                    None
                }
            }
        }
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A recorded event entry for logging and replay.
#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub time: Time,
    pub event_id: String,
    pub event_type: EventType,
    pub payload: Option<SignalValue>,
}

/// Event logger for debugging, statistics, and replay.
#[derive(Debug, Default)]
pub struct EventLogger {
    pub log: Vec<EventLogEntry>,
}

impl EventLogger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, time: Time, event: &Event) {
        self.log.push(EventLogEntry {
            time,
            event_id: event.id.clone(),
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
        });
    }

    pub fn clear(&mut self) {
        self.log.clear();
    }

    pub fn count(&self) -> usize {
        self.log.len()
    }

    pub fn events_since(&self, time: Time) -> impl Iterator<Item = &EventLogEntry> {
        self.log.iter().filter(move |e| e.time >= time)
    }
}
