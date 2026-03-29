//! Memphis integration for ML runtime.
//!
//! Bridges ML execution to Memphis journal/chains/decision logging.
//!
//! Every gate/sensor/actuator operation is written to an immutable,
//! signed `journal.chain` file, and notable decisions are recorded
//! in `decisions.chain`.
//!
//! ## Architecture
//!
//! ```text
//! UserCode → TracedRuntime → Runtime<M>
//!                           → MemphisRuntimeBridge
//!                              → journal.chain
//!                              → decisions.chain
//! ```
//!
//! ## Quick Start
//!
//! ```ignore
//! use ml_core::{MockMachine, MLExpr};
//! use ml_memphis::TracedRuntime;
//!
//! let machine = MockMachine::new();
//! let mut rt = TracedRuntime::new(
//!     machine,
//!     "/tmp/memphis-data",
//!     "test-machine",
//!     "my-program-v1",
//! )?;
//!
//! let expr = MLExpr::parse("(gate garage on)")?;
//! rt.execute_sync(expr)?;
//! ```
//!
//! ## File-Based Backend
//!
//! By default, the bridge writes to local files. This works without
//! a Memphis server and serves as a local audit log:
//!
//! - `journal.chain` — one JSON+SHA256 entry per ML operation
//! - `decisions.chain` — one JSON+SHA256 entry per notable decision
//!
//! Set the `MEMPHIS_STATION_URL` env var to connect to a live Memphis
//! station (future: native Memphis backend).

pub mod journal;
pub mod decisions;
pub mod runtime;
pub mod tracer;

pub use journal::{JournalEntry, JournalWriter};
pub use decisions::{DecisionEntry, DecisionWriter, DecisionError};
pub use runtime::{MemphisRuntimeBridge, BridgeError, BridgeFallback};
pub use tracer::{TracedRuntime, traced};
