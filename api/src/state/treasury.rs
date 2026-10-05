use steel::*;

use crate::{consts::*, math::add, state::GridAccount};

/// Singleton holding protocol SOL (as lamports) and owning every program token account.
/// Invariant: lamports >= rent + admin_sol + buyback_sol + vault_sol + team_sol + sum(pot_sol).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Treasury {
    /// Admin fee, paid out by WithdrawTeam (Reset never sends to a collector).
    pub admin_sol: u64,
    /// Burn + recycle share, swapped to `$GRID` by Buyback.
    pub buyback_sol: u64,
    /// Swapped into vault assets by BuyAsset(vault).
    pub vault_sol: u64,
    pub team_sol: u64,
    /// SOL accrued per tile, swapped into `pot_tokens` by BuyAsset(pot).
    pub pot_sol: [u64; TILES],
    pub pot_tokens: [u64; TILES],
    /// Vault holdings (0..25 tile assets, 25 = `$GRID`).
    pub vault: [u64; 26],
    /// SOL spent buying each tile's vault asset.
    pub vault_spent: [u64; TILES],
    /// Tile tokens handed to unclosed rounds (pots and vault hits) and not yet paid.
    /// SetTile waits for 0 so those rounds keep paying the mint they were bought in.
    pub owed: [u64; TILES],
    /// Unallocated `$GRID` emission reserve.
    pub reserve: u64,
    /// Refining accumulator: refined `$GRID` per unit of unrefined.
    pub rewards_factor: Numeric,
    pub total_refined: u64,
    pub total_unrefined: u64,
}

impl Treasury {
    /// Lamports the Treasury owes to its SOL buckets (excludes rent).
    pub fn sol_books(&self) -> Result<u64, ProgramError> {
        [self.admin_sol, self.buyback_sol, self.vault_sol, self.team_sol]
            .iter()
            .chain(&self.pot_sol)
            .try_fold(0, |sum, v| add(sum, *v))
    }
}

account!(GridAccount, Treasury);
