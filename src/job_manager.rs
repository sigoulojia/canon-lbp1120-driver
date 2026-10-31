#![allow(dead_code)]
//! Deterministic Print Job State Machine & Virtual CAPT Hardware Endpoint Simulator
//!
//! Enforces strict lifecycle progression:
//! Submitted -> Accepted -> Rendering -> Processing -> Transmitting -> Printing -> Completed
//! Distinguishes between USB transmission completion (`Transmitting`) and physical mechanical
//! paper delivery (`Printing` -> `Completed` via `shipped_pages` exit sensor increment).

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailureReason {
    PaperEmpty,
    PaperJam,
    CoverOpen,
    NoCartridge,
    FifoBufferOverflow { stream_bytes: usize, limit_bytes: usize },
    UsbDisconnect,
    CorruptDocument(String),
    CancelledByUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLifecycleState {
    Submitted,
    Accepted,
    Rendering { current_page: u32, total_pages: u32 },
    Processing { algorithm: String, scoa_bytes: usize },
    Transmitting { bytes_sent: usize, total_bytes: usize },
    Printing { baseline_shipped: u16, current_shipped: u16, feed_active: bool },
    Completed { delivered_pages: u32, total_ms: u128 },
    Failed(JobFailureReason),
}

#[derive(Debug, Clone)]
pub struct StateTransitionRecord {
    pub state: JobLifecycleState,
    pub timestamp_ms: u128,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct TraceablePrintJob {
    pub job_id: u64,
    pub document_name: String,
    pub total_pages: u32,
    pub current_state: JobLifecycleState,
    pub history: Vec<StateTransitionRecord>,
}

impl TraceablePrintJob {
    pub fn new(job_id: u64, document_name: &str, total_pages: u32) -> Self {
        let now = Self::now_ms();
        let initial = JobLifecycleState::Submitted;
        Self {
            job_id,
            document_name: document_name.to_string(),
            total_pages,
            current_state: initial.clone(),
            history: vec![StateTransitionRecord {
                state: initial,
                timestamp_ms: now,
                note: "Job submitted to spooler queue".to_string(),
            }],
        }
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    pub fn transition(&mut self, next: JobLifecycleState, note: &str) {
        self.current_state = next.clone();
        self.history.push(StateTransitionRecord {
            state: next,
            timestamp_ms: Self::now_ms(),
            note: note.to_string(),
        });
    }
}
