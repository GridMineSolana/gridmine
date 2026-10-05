use steel::*;

use crate::{
    consts::*,
    math::{add, mul_div, sub},
    state::{GridAccount, Treasury},
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Miner {
    pub authority: Pubkey,
    /// Last round this miner was checkpointed for.
    pub checkpoint_id: u64,
    /// Lamports held back to pay a bot that checkpoints this miner.
    pub checkpoint_fee: u64,
    /// SOL deployed on each tile in `round_id`.
    pub deployed: [u64; TILES],
    /// SOL on each tile before this miner deployed (solo winner sampling).
    pub cumulative: [u64; TILES],
    pub round_id: u64,
    /// Treasury rewards factor at the last update.
    pub rewards_factor: Numeric,
    /// SOL owed to this miner (fallback when it could not be sent at checkpoint).
    pub rewards_sol: u64,
    /// `$GRID` earned from other miners' refining fees (claimable fee-free).
    pub refined: u64,
    /// `$GRID` mined (claiming pays the refining fee).
    pub unrefined: u64,
    pub last_claim_token_at: i64,
    pub last_claim_sol_at: i64,
    pub lifetime_token: u64,
    pub lifetime_deployed: u64,
    pub lifetime_sol: u64,
}

impl Miner {
    /// Claims `bps` of refined + unrefined `$GRID`. Returns (amount paid out, refining fee).
    /// F15: the fee goes only to *other* unrefined holders; the claimer's remaining unrefined
    /// is excluded from the denominator and its factor is advanced past this distribution.
    pub fn claim_token(
        &mut self,
        now: i64,
        treasury: &mut Treasury,
        bps: u64,
        refine_fee_bps: u64,
    ) -> Result<(u64, u64), ProgramError> {
        self.update_rewards(treasury);

        let bps = bps.min(DENOMINATOR_BPS);
        let claim_refined = mul_div(self.refined, bps, DENOMINATOR_BPS)?;
        let claim_unrefined = mul_div(self.unrefined, bps, DENOMINATOR_BPS)?;
        self.refined = sub(self.refined, claim_refined)?;
        self.unrefined = sub(self.unrefined, claim_unrefined)?;
        treasury.total_refined = sub(treasury.total_refined, claim_refined)?;
        treasury.total_unrefined = sub(treasury.total_unrefined, claim_unrefined)?;
        self.last_claim_token_at = now;

        let others = sub(treasury.total_unrefined, self.unrefined)?;
        let mut fee = 0;
        if claim_unrefined > 0 && others > 0 {
            fee = mul_div(claim_unrefined, refine_fee_bps, DENOMINATOR_BPS)?;
            if fee > 0 {
                treasury.rewards_factor += Numeric::from_fraction(fee, others);
                treasury.total_refined = add(treasury.total_refined, fee)?;
                self.rewards_factor = treasury.rewards_factor;
                self.lifetime_token = self.lifetime_token.saturating_sub(fee);
            }
        }
        Ok((sub(add(claim_refined, claim_unrefined)?, fee)?, fee))
    }

    pub fn claim_sol(&mut self, now: i64) -> u64 {
        self.last_claim_sol_at = now;
        std::mem::take(&mut self.rewards_sol)
    }

    /// Credits refined `$GRID` accrued since the last update.
    pub fn update_rewards(&mut self, treasury: &Treasury) {
        if treasury.rewards_factor > self.rewards_factor {
            let accrued = (treasury.rewards_factor - self.rewards_factor)
                * Numeric::from_u64(self.unrefined);
            self.refined = self.refined.saturating_add(accrued.to_u64());
            self.lifetime_token = self.lifetime_token.saturating_add(accrued.to_u64());
        }
        self.rewards_factor = treasury.rewards_factor;
    }
}

account!(GridAccount, Miner);
