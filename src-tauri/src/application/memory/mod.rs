//! Memory (ADR 0037): durable lessons recovered into a prompt as context, never as instruction.

pub mod block;
pub mod ports;
pub mod proposal;
pub mod recall;
pub mod service;

pub use block::MemoryBlock;
pub use ports::{MemoryStore, ProjectProbe};
pub use recall::{Recall, RecallRequest};
pub use service::{Decision, MemoryService, NewMemory, SubmitContext, Validator};
