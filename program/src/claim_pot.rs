use grid_api::prelude::*;
use steel::*;

use crate::token::{ensure_ata, send_from_treasury};

/// Permissionless. Sends a round's pot to `round.pot_winner`'s ATA (created if needed, signer pays).
pub fn process_claim_pot(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<ClaimPot>(data)?;
    let [signer_info, config_info, round_info, treasury_info, mint_info, treasury_tokens_info, winner_info, winner_tokens_info, system_program, token_program, ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    let round = round_info
        .is_writable()?
        .has_seeds(&[ROUND, &args.round_id.to_le_bytes()], &grid_api::ID)?
        .as_account_mut::<Round>(&grid_api::ID)?;
    if round.pot_paid != 0 {
        return Err(GridError::AlreadyClaimed.into());
    }
    if round.pot_amount == 0 || round.pot_winner == Pubkey::default() {
        return Err(GridError::NothingToClaim.into());
    }
    // pot_amount > 0 only on settled rounds, so winning_tile < 25.
    let w = round.winning_tile as usize;
    let tile = config.tiles[w];
    mint_info.has_address(&tile.mint)?.has_owner(&tile.token_program())?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    treasury_tokens_info
        .is_writable()?
        .as_associated_token_account(treasury_info.key, mint_info.key)?;
    winner_info.has_address(&round.pot_winner)?;
    winner_tokens_info.is_writable()?;
    system_program.is_program(&system_program::ID)?;
    token_program.is_program(&tile.token_program())?;
    ata_program.is_program(&spl_associated_token_account::ID)?;

    ensure_ata(signer_info, winner_info, winner_tokens_info, mint_info, system_program, token_program, ata_program)?;
    round.pot_paid = round.pot_amount;
    treasury.owed[w] = sub(treasury.owed[w], round.pot_amount)?;
    send_from_treasury(
        treasury_info,
        treasury_tokens_info,
        mint_info,
        winner_tokens_info,
        token_program,
        round.pot_amount,
        tile.decimals,
    )
}
