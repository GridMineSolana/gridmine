use grid_api::prelude::*;
use solana_program::rent::Rent;
use steel::*;

/// Closes an expired round: unclaimed SOL to the buyback bucket, uncredited `$GRID` back to the
/// reserve, unpaid pot and vault shares back to the Treasury books (the tokens never left its
/// ATAs), rent to the rent payer.
pub fn process_close(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let [signer_info, board_info, rent_payer_info, round_info, treasury_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let board = board_info
        .has_seeds(&[BOARD], &grid_api::ID)?
        .as_account::<Board>(&grid_api::ID)?;
    rent_payer_info.is_writable()?;
    let round = round_info
        .is_writable()?
        .as_account::<Round>(&grid_api::ID)?
        .assert_err(|r| r.id < board.round_id, GridError::RoundNotOver.into())?
        .assert_err(|r| r.expires_at < clock.slot, GridError::RoundNotOver.into())?
        .assert_err(|r| r.rent_payer == *rent_payer_info.key, GridError::NotAuthorized.into())?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;

    let unclaimed_sol = sub(round_info.lamports(), Rent::get()?.minimum_balance(Round::SIZE))?;
    treasury.buyback_sol = add(treasury.buyback_sol, unclaimed_sol)?;
    treasury.reserve = add(treasury.reserve, sub(round.token_reward, round.token_credited)?)?;
    if round.pot_amount > 0 {
        let w = round.winning_tile as usize;
        let left = sub(round.pot_amount, round.pot_paid)?;
        treasury.pot_tokens[w] = add(treasury.pot_tokens[w], left)?;
        treasury.owed[w] = sub(treasury.owed[w], left)?;
    }
    if round.vault_hit != 0 {
        for i in 0..=VAULT_GRID {
            let left = sub(round.vault[i], round.vault_paid[i])?;
            treasury.vault[i] = add(treasury.vault[i], left)?;
            if i < TILES {
                treasury.owed[i] = sub(treasury.owed[i], left)?;
            }
        }
    }
    round_info.send(unclaimed_sol, treasury_info);
    round_info.close(rent_payer_info)?;
    Ok(())
}
