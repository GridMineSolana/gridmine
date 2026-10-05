use steel::*;

use crate::state::GridAccount;

/// Singleton tracking the current round.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Board {
    pub round_id: u64,
    pub start_slot: u64,
    /// u64::MAX until the round's first deploy.
    pub end_slot: u64,
}

account!(GridAccount, Board);
