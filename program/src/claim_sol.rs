use grid_api::prelude::*;
use steel::*;

/// Claims SOL held in the miner account (only non-zero after a Checkpoint fallback).
pub fn process_claim_sol(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let [signer_info, miner_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let miner = miner_info
        .is_writable()?
        .has_seeds(&[MINER, &signer_info.key.to_bytes()], &grid_api::ID)?
        .as_account_mut::<Miner>(&grid_api::ID)?;

    let amount = miner.claim_sol(clock.unix_timestamp);
    miner_info.send(amount, signer_info);

    ClaimEvent {
        disc: GridEvent::Claim as u64,
        authority: miner.authority,
        amount,
        claim_type: 0,
        ts: clock.unix_timestamp,
    }
    .log();
    Ok(())
}
