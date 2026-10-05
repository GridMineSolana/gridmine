use grid_api::prelude::*;
use steel::*;

use crate::token::{ensure_ata, send_from_treasury};

/// Claims `bps` of the miner's `$GRID`. Unrefined `$GRID` pays the refining fee (F15-fixed).
pub fn process_claim_token(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<ClaimToken>(data)?;
    let clock = Clock::get()?;
    let [signer_info, config_info, miner_info, mint_info, recipient_info, treasury_info, treasury_tokens_info, system_program, token_program, ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    let miner = miner_info
        .is_writable()?
        .has_seeds(&[MINER, &signer_info.key.to_bytes()], &grid_api::ID)?
        .as_account_mut::<Miner>(&grid_api::ID)?;
    mint_info.has_address(&config.token_mint)?.has_owner(&config.token_program)?;
    recipient_info.is_writable()?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    treasury_tokens_info
        .is_writable()?
        .as_associated_token_account(treasury_info.key, mint_info.key)?;
    system_program.is_program(&system_program::ID)?;
    token_program.is_program(&config.token_program)?;
    ata_program.is_program(&spl_associated_token_account::ID)?;

    ensure_ata(signer_info, signer_info, recipient_info, mint_info, system_program, token_program, ata_program)?;

    let (amount, _fee) =
        miner.claim_token(clock.unix_timestamp, treasury, args.bps, config.params.refine_fee_bps)?;
    send_from_treasury(
        treasury_info,
        treasury_tokens_info,
        mint_info,
        recipient_info,
        token_program,
        amount,
        config.token_decimals as u8,
    )?;

    ClaimEvent {
        disc: GridEvent::Claim as u64,
        authority: miner.authority,
        amount,
        claim_type: 1,
        ts: clock.unix_timestamp,
    }
    .log();
    Ok(())
}
