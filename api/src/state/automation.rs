use steel::*;

use crate::{consts::DENOMINATOR_BPS, error::GridError, state::GridAccount};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Automation {
    /// SOL to deploy on each tile per round.
    pub amount: u64,
    pub authority: Pubkey,
    /// SOL left to deploy.
    pub balance: u64,
    pub executor: Pubkey,
    /// Executor fee (lamports, or bps for DiscretionaryBps).
    pub fee: u64,
    pub strategy: u64,
    /// Tiles to deploy on (Preferred), or popcount = how many tiles (Random).
    pub mask: u64,
    /// 1 = send SOL winnings back into `balance`.
    pub reload: u64,
    pub total_sol_spent: u64,
    pub total_token_earned: u64,
    /// Random strategy: number of solo / split tiles to target (0 = no preference).
    pub solo_tiles: u64,
    pub split_tiles: u64,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum AutomationStrategy {
    Random = 0,
    Preferred = 1,
    Discretionary = 2,
    DiscretionaryBps = 3,
}

impl AutomationStrategy {
    /// F14: typed error instead of a panic on a bad byte.
    pub fn parse(value: u64) -> Result<Self, ProgramError> {
        u8::try_from(value)
            .ok()
            .and_then(|v| Self::try_from(v).ok())
            .ok_or(GridError::InvalidStrategy.into())
    }
}

impl Automation {
    pub fn min_fee(&self, deploy_amount: u64) -> u64 {
        if self.strategy == AutomationStrategy::DiscretionaryBps as u64 {
            (deploy_amount as u128 * self.fee as u128 / DENOMINATOR_BPS as u128) as u64
        } else {
            self.fee
        }
    }
}

account!(GridAccount, Automation);
