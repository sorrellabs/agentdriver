//! One adapter per terminal host, each behind a feature.

#[cfg(feature = "fake")]
pub mod fake;
#[cfg(feature = "herdr")]
pub mod herdr;
#[cfg(feature = "tmux")]
pub mod tmux;
