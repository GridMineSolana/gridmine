use grid_api::prelude::*;
use solana_program::rent::Rent;
use steel::*;

use crate::swap::{check_worker, swap, treasury_ata_amount, SwapAccounts};

/// Worker only. Swaps `amount_in` into `$GRID`: first from the buyback bucket (protocol part),
/// then creator fees (the Treasury wSOL ATA, then Treasury lamports above rent and the books).
/// The protocol part is burned / recycled per `burn_bps : recycle_bps`; the creator part goes
/// 100% to the reserve.
pub fn process_buyback(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let (args, route_data) = parse_args_with_route::<Buyback>(data)?;
    let [signer_info, config_info, treasury_info, treasury_wsol_info, swap_info, swap_wsol_info, mint_info, dest_info, wsol_program, token_program, jupiter_program, route @ ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = config_info
        .is_writable()?
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account_mut::<Config>(&grid_api::ID)?;
    check_worker(signer_info, config, args.amount_in)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    let creator_wsol = treasury_wsol_info.is_writable()?.has_owner(&spl_token::ID)?;
    let creator_wsol = treasury_ata_amount(creator_wsol, treasury_info.key, &SOL_MINT)?;
    mint_info.is_writable()?.has_address(&config.token_mint)?.has_owner(&config.token_program)?;
    token_program.is_program(&config.token_program)?;

    // Sources, in order: buyback bucket, Treasury wSOL ATA, Treasury surplus lamports.
    let protocol = args.amount_in.min(treasury.buyback_sol);
    let creator = sub(args.amount_in, protocol)?;
    let from_wsol = creator.min(creator_wsol);
    let from_surplus = sub(creator, from_wsol)?;
    let rent = Rent::get()?.minimum_balance(treasury_info.data_len());
    let surplus = sub(treasury_info.lamports(), add(rent, treasury.sol_books()?)?)?;
    if from_surplus > surplus {
        return Err(GridError::InsufficientFunds.into());
    }
    treasury.buyback_sol = sub(treasury.buyback_sol, protocol)?;

    let accounts = SwapAccounts {
        treasury: treasury_info,
        swap: swap_info,
        swap_wsol: swap_wsol_info,
        dest: dest_info,
        wsol_program,
        jupiter: jupiter_program,
        route,
    };
    let received = swap(
        &accounts,
        &config.token_mint,
        &config.token_program,
        add(protocol, from_surplus)?,
        Some((treasury_wsol_info, from_wsol)),
        args.min_out,
        route_data,
    )?;

    // Output pro-rata to inputs; the protocol part splits burn / recycle.
    let p = &config.params;
    let protocol_out = mul_div(received, protocol, args.amount_in)?;
    let split = add(p.burn_bps, p.recycle_bps)?;
    let burn = if split == 0 { 0 } else { mul_div(protocol_out, p.burn_bps, split)? };
    treasury.reserve = add(treasury.reserve, sub(received, burn)?)?;
    if burn > 0 {
        burn_checked_signed(
            dest_info,
            mint_info,
            treasury_info,
            token_program,
            burn,
            config.token_decimals as u8,
            &[TREASURY],
        )?;
    }
    Ok(())
}
