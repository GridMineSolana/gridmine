use grid_api::prelude::*;
use steel::*;

use crate::admin::load_config_as_admin;
use crate::token::{ensure_ata, send_from_treasury};

/// Admin or worker. Adds SOL or tokens to a Treasury bucket. Token buckets credit the amount actually received.
pub fn process_fund(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<Fund>(data)?;
    let [signer_info, config_info, treasury_info, system_program, tokens @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    if *signer_info.key != config.admin && *signer_info.key != config.worker {
        return Err(GridError::NotAuthorized.into());
    }
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    system_program.is_program(&system_program::ID)?;
    if args.amount == 0 {
        return Err(GridError::AmountTooSmall.into());
    }

    let (balance, token) = bucket(treasury, config, args.bucket, args.index)?;
    let received = match token {
        None => {
            treasury_info.collect(args.amount, signer_info)?;
            args.amount
        }
        Some((mint, program, decimals)) => {
            let [mint_info, from_info, treasury_tokens_info, token_program, ata_program] = tokens else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            mint_info.has_address(&mint)?;
            from_info.is_writable()?;
            treasury_tokens_info.is_writable()?;
            token_program.is_program(&program)?;
            ata_program.is_program(&spl_associated_token_account::ID)?;
            ensure_ata(signer_info, treasury_info, treasury_tokens_info, mint_info, system_program, token_program, ata_program)?;
            let before = treasury_tokens_info.as_token_account()?.amount();
            transfer_checked(signer_info, from_info, mint_info, treasury_tokens_info, token_program, args.amount, decimals)?;
            sub(treasury_tokens_info.as_token_account()?.amount(), before)?
        }
    };
    *balance = add(*balance, received)?;
    Ok(())
}

/// Admin. Sends `amount` out of a Treasury bucket to any wallet (SOL) or token account (tokens).
pub fn process_withdraw(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<Withdraw>(data)?;
    let [signer_info, config_info, treasury_info, to_info, tokens @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = load_config_as_admin(signer_info, config_info)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    to_info.is_writable()?;

    let (balance, token) = bucket(treasury, config, args.bucket, args.index)?;
    *balance = balance.checked_sub(args.amount).ok_or(GridError::InsufficientFunds)?;
    match token {
        None => treasury_info.send(args.amount, to_info),
        Some((mint, program, decimals)) => {
            let [mint_info, treasury_tokens_info, token_program] = tokens else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            mint_info.has_address(&mint)?;
            token_program.is_program(&program)?;
            treasury_tokens_info.is_writable()?.as_associated_token_account(treasury_info.key, &mint)?;
            send_from_treasury(treasury_info, treasury_tokens_info, mint_info, to_info, token_program, args.amount, decimals)?;
        }
    }
    Ok(())
}

/// A bucket's balance and, for token buckets, its (mint, token program, decimals).
type Token = Option<(Pubkey, Pubkey, u8)>;
fn bucket<'a>(t: &'a mut Treasury, c: &Config, bucket: u64, index: u64) -> Result<(&'a mut u64, Token), ProgramError> {
    let i = usize::try_from(index).map_err(|_| ProgramError::from(GridError::InvalidTile))?;
    let tile = |i: usize| -> Result<Token, ProgramError> {
        let tile = c.tiles.get(i).filter(|t| t.mint != Pubkey::default()).ok_or(GridError::InvalidTile)?;
        Ok(Some((tile.mint, tile.token_program(), tile.decimals)))
    };
    let grid = Some((c.token_mint, c.token_program, c.token_decimals as u8));
    Ok(match bucket {
        BUCKET_ADMIN_SOL => (&mut t.admin_sol, None),
        BUCKET_BUYBACK_SOL => (&mut t.buyback_sol, None),
        BUCKET_VAULT_SOL => (&mut t.vault_sol, None),
        BUCKET_TEAM_SOL => (&mut t.team_sol, None),
        BUCKET_POT_SOL => {
            tile(i)?;
            (&mut t.pot_sol[i], None)
        }
        BUCKET_VAULT_TOKENS if i == VAULT_GRID => (&mut t.vault[i], grid),
        BUCKET_VAULT_TOKENS => (&mut t.vault[i], tile(i)?),
        BUCKET_POT_TOKENS => {
            let token = tile(i)?;
            (&mut t.pot_tokens[i], token)
        }
        BUCKET_RESERVE => (&mut t.reserve, grid),
        _ => return Err(ProgramError::InvalidInstructionData),
    })
}
