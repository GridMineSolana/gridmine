use grid_api::prelude::*;
use steel::*;

use crate::token::{ensure_ata, send_from_treasury};

/// Pays the signer's pro-rata share of one vault entry (`index` 25 = `$GRID`) of a hit round:
/// `round.vault[index] * ticket.share / deployed[winning tile]`. One mint per call, so a paused or
/// frozen mint only blocks its own claim. Closes the ticket when every entry is claimed, or when
/// the round was already closed (its unclaimed shares went back to the Vault).
pub fn process_claim_vault(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<ClaimVault>(data)?;
    let [signer_info, config_info, round_info, ticket_info, treasury_info, mint_info, treasury_tokens_info, recipient_info, system_program, token_program, ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    let round_id = args.round_id.to_le_bytes();
    let ticket = ticket_info
        .is_writable()?
        .has_seeds(&[TICKET, &round_id, &signer_info.key.to_bytes()], &grid_api::ID)?
        .as_account_mut::<VaultTicket>(&grid_api::ID)?;
    round_info.has_seeds(&[ROUND, &round_id], &grid_api::ID)?;
    if round_info.data_is_empty() {
        return ticket_info.close(signer_info);
    }
    let round = round_info.is_writable()?.as_account_mut::<Round>(&grid_api::ID)?;
    let i = usize::try_from(args.index).ok().filter(|i| *i <= VAULT_GRID).ok_or(GridError::InvalidTile)?;
    if ticket.claimed & (1 << i) != 0 {
        return Err(GridError::AlreadyClaimed.into());
    }
    let (mint, program, decimals) = if i == VAULT_GRID {
        (config.token_mint, config.token_program, config.token_decimals as u8)
    } else {
        let t = config.tiles[i];
        (t.mint, t.token_program(), t.decimals)
    };
    mint_info.has_address(&mint)?.has_owner(&program)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    treasury_tokens_info
        .is_writable()?
        .as_associated_token_account(treasury_info.key, &mint)?;
    recipient_info.is_writable()?;
    system_program.is_program(&system_program::ID)?;
    token_program.is_program(&program)?;
    ata_program.is_program(&spl_associated_token_account::ID)?;

    // Tickets exist only on hit rounds, and only for miners on the winning tile.
    let amount = mul_div(round.vault[i], ticket.share, round.deployed[round.winning_tile as usize])?;
    ensure_ata(signer_info, signer_info, recipient_info, mint_info, system_program, token_program, ata_program)?;
    round.vault_paid[i] = add(round.vault_paid[i], amount)?;
    if i < TILES {
        treasury.owed[i] = sub(treasury.owed[i], amount)?;
    }
    send_from_treasury(treasury_info, treasury_tokens_info, mint_info, recipient_info, token_program, amount, decimals)?;

    ticket.claimed |= 1 << i;
    if ticket.claimed == ALL_CLAIMED {
        ticket_info.close(signer_info)?;
    }
    Ok(())
}
