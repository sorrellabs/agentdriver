//! Typed identifiers. Hosts define the contents; agentdriver treats them as opaque.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// One agentdriver session.
    SessionId
);
string_id!(
    /// A pane as named by its host. Hosts may reuse these.
    PaneId
);
string_id!(
    /// The terminal behind a pane. Distinguishes a reused pane id.
    TerminalId
);

macro_rules! open_kind {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Cow<'static, str>);

        impl $name {
            pub const fn from_static(value: &'static str) -> Self {
                Self(Cow::Borrowed(value))
            }

            pub fn new(value: impl Into<String>) -> Self {
                Self(Cow::Owned(value.into()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

open_kind!(
    /// Names a terminal host. Open, so a host can live outside this crate.
    HostKind
);
open_kind!(
    /// Names an agent harness. Open, so a harness can live outside this crate.
    HarnessKind
);
