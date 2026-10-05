use grid_api::prelude::*;
use steel::*;

use crate::swap::{check_worker, swap, SwapAccounts};

/// Worker only. Swaps `amount_in` of tile `tile`'s pot SOL (or of the vault SOL) into that tile's
/// asset, held in the Treasury's ATA (which must exist).
pub fn process_buy_asset(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let (args, route_data) = parse_args_with_route::<BuyAsset>(data)?;
    let [signer_info, config_info, treasury_info, swap_info, swap_wsol_info, dest_info, wsol_program, jupiter_program, route @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = config_info
        .is_writable()?
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account_mut::<Config>(&grid_api::ID)?;
    check_worker(signer_info, config, args.amount_in)?;
    let i = usize::try_from(args.tile).ok().filter(|i| *i < TILES).ok_or(GridError::InvalidTile)?;
    let tile = config.tiles[i];
    if tile.mint == Pubkey::default() {
        return Err(GridError::InvalidTile.into());
    }
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;

    let bucket = match args.kind {
        BUY_POT => &mut treasury.pot_sol[i],
        BUY_VAULT => &mut treasury.vault_sol,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    *bucket = bucket.checked_sub(args.amount_in).ok_or(GridError::InsufficientFunds)?;

    let accounts = SwapAccounts {
        treasury: treasury_info,
        swap: swap_info,
        swap_wsol: swap_wsol_info,
        dest: dest_info,
        wsol_program,
        jupiter: jupiter_program,
        route,
    };
    let received = swap(&accounts, &tile.mint, &tile.token_program(), args.amount_in, None, args.min_out, route_data)?;

    if args.kind == BUY_POT {
        treasury.pot_tokens[i] = add(treasury.pot_tokens[i], received)?;
    } else {
        treasury.vault[i] = add(treasury.vault[i], received)?;
        treasury.vault_spent[i] = add(treasury.vault_spent[i], args.amount_in)?;
    }
    Ok(())
}
