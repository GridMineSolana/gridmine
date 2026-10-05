use steel::*;

use crate::{consts::*, error::GridError, state::GridAccount};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Config {
    /// Squads multisig after launch. Signs UpdateConfig, SetTile, SetPause, WithdrawTeam.
    pub admin: Pubkey,
    /// Hot key. Signs BuyAsset and Buyback only.
    pub worker: Pubkey,
    /// Receives the admin fee.
    pub admin_collector: Pubkey,
    /// Receives WithdrawTeam.
    pub team_collector: Pubkey,
    /// `$GRID` mint, fixed at Initialize.
    pub token_mint: Pubkey,
    pub token_program: Pubkey,
    pub tiles: [Tile; TILES],
    pub token_decimals: u64,
    /// 1 = Deploy blocked.
    pub paused: u64,
    /// Slot of the last worker swap (swap interval guard).
    pub last_swap_slot: u64,
    pub params: Params,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Tile {
    pub mint: Pubkey,
    pub is_t22: u8,
    pub decimals: u8,
    pub _pad: [u8; 6],
}

impl Tile {
    pub fn token_program(&self) -> Pubkey {
        if self.is_t22 != 0 {
            spl_token_2022::ID
        } else {
            spl_token::ID
        }
    }
}

/// Tunables (PLAN §4). Set at Initialize, changed via UpdateConfig.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Params {
    pub round_slots: u64,
    pub intermission_slots: u64,
    /// Lamports per tile.
    pub min_deploy: u64,
    /// Of all SOL deployed.
    pub admin_fee_bps: u64,
    /// Of (losing tile - admin fee).
    pub protocol_fee_bps: u64,
    /// Protocol fee split; must sum to 10,000.
    pub burn_bps: u64,
    pub recycle_bps: u64,
    pub pots_bps: u64,
    pub vault_bps: u64,
    pub team_bps: u64,
    /// Emission per round, parts per billion of the reserve.
    pub emission_ppb: u64,
    /// Emission cap per round, `$GRID` base units.
    pub max_emission: u64,
    /// Emission multiplier while `now < boost_until` (10,000 = 1x).
    pub boost_bps: u64,
    pub boost_until: i64,
    /// Share of each emission that goes to the Vault.
    pub vault_emission_bps: u64,
    /// The Vault hits with odds 1 in `vault_odds`.
    pub vault_odds: u64,
    pub refine_fee_bps: u64,
    pub void_after_slots: u64,
    pub rerequest_after_slots: u64,
    pub max_swap_lamports: u64,
    pub min_swap_interval_slots: u64,
}

impl Params {
    pub fn validate(&self) -> Result<(), ProgramError> {
        let split = [self.burn_bps, self.recycle_bps, self.pots_bps, self.vault_bps, self.team_bps];
        let ok = split.iter().map(|b| *b as u128).sum::<u128>() == DENOMINATOR_BPS as u128
            && self.round_slots >= 10
            && self.intermission_slots >= 1
            && self.min_deploy >= 100_000
            && self.admin_fee_bps <= 500
            && self.protocol_fee_bps <= 2_000
            && self.emission_ppb <= 30_000
            && (DENOMINATOR_BPS..=3 * DENOMINATOR_BPS).contains(&self.boost_bps)
            && self.vault_emission_bps <= 5_000
            && self.vault_odds >= 1
            && self.refine_fee_bps <= 2_000
            && self.void_after_slots > self.intermission_slots
            && self.rerequest_after_slots >= 1
            && self.max_swap_lamports >= 1;
        if ok {
            Ok(())
        } else {
            Err(GridError::InvalidConfig.into())
        }
    }
}

account!(GridAccount, Config);
