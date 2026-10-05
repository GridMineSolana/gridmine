use grid_api::prelude::*;
use steel::*;

use crate::token::ensure_ata;

/// Deposits `$GRID` into the emission reserve. Credits the amount actually received.
pub fn process_fund_reserve(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<FundReserve>(data)?;
    let [signer_info, config_info, sender_info, mint_info, treasury_info, treasury_tokens_info, system_program, token_program, ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    mint_info.has_address(&config.token_mint)?.has_owner(&config.token_program)?;
    sender_info.is_writable()?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    treasury_tokens_info.is_writable()?;
    system_program.is_program(&system_program::ID)?;
    token_program.is_program(&config.token_program)?;
    ata_program.is_program(&spl_associated_token_account::ID)?;
    if args.amount == 0 {
        return Err(GridError::AmountTooSmall.into());
    }

    ensure_ata(signer_info, treasury_info, treasury_tokens_info, mint_info, system_program, token_program, ata_program)?;
    let before = treasury_tokens_info.as_token_account()?.amount();
    transfer_checked(
        signer_info,
        sender_info,
        mint_info,
        treasury_tokens_info,
        token_program,
        args.amount,
        config.token_decimals as u8,
    )?;
    let received = sub(treasury_tokens_info.as_token_account()?.amount(), before)?;
    treasury.reserve = add(treasury.reserve, received)?;
    Ok(())
}
