use steel::*;

use crate::state::GridAccount;

/// A winning-tile miner's claim on a vault-hit round (`[ticket, round_id, authority]`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct VaultTicket {
    pub authority: Pubkey,
    pub round_id: u64,
    /// Lamports the miner deployed on the winning tile.
    pub share: u64,
    /// Bit i set = vault entry i (0..25 tiles, 25 = `$GRID`) claimed.
    pub claimed: u64,
}

account!(GridAccount, VaultTicket);
