//! Newtype identifiers. Wrapping `Uuid` keeps a `SeatId` from being confused
//! with a `SessionId` at a call site, and gives every id uniform serde.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_newtype {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            /// Generate a fresh random (v4) id.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// The wrapped raw UUID.
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(u: Uuid) -> Self {
                Self(u)
            }
        }
    };
}

uuid_newtype!(
    /// Identifies a configured seat (panelist or mediator) within a session.
    SeatId
);
uuid_newtype!(
    /// Identifies a deliberation session.
    SessionId
);
uuid_newtype!(
    /// Identifies a round (including the synthetic `kind=finalization` round).
    RoundId
);
uuid_newtype!(
    /// Identifies a single provider attempt for one seat in one round.
    AttemptId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        assert_ne!(SeatId::new(), SeatId::new());
    }

    #[test]
    fn serde_is_transparent_string() {
        let id = SeatId::new();
        let json = serde_json::to_string(&id).unwrap();
        // Transparent: serializes as a bare quoted uuid, not a wrapper object.
        assert_eq!(json, format!("\"{}\"", id.0));
        let back: SeatId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
