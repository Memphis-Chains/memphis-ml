//! Memphis integration for ML runtime.
//!
//! Bridges ML execution to Memphis journal/chains/decision logging.
//!
//! ## Architecture
//!
//! ```text
//! UserCode → TracedRuntime → Runtime<HalMachine>
//!                           → MemphisRuntimeBridge
//!                              → journal.chain
//!                              → decisions.chain
//! ```

pub mod journal;
pub mod decisions;
pub mod runtime;
pub mod tracer;

pub use journal::JournalWriter;
pub use decisions::DecisionWriter;
pub use runtime::MemphisRuntimeBridge;
pub use tracer::TracedRuntime;
