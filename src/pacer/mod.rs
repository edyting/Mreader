//! Visual pacing engine.

pub mod engine;
pub mod modes;
pub mod timing;

pub use engine::{PaceSnapshot, PaceState, PacerEngine};
pub use modes::PacingMode;
pub use timing::{ms_per_phrase, ms_per_word, PaceClock};
