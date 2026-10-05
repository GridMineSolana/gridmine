use grid_api::prelude::*;
use solana_program::rent::Rent;
use steel::*;

/// Credits a miner's SOL returns and `$GRID` for its last round, sets the pot winner if the
/// `draw(rng, "pot")` sample lands on this miner, and opens a VaultTicket on hit rounds (rent
/// from winnings). Permissionless; bots earn the checkpoint fee in the last 12h before expiry.
pub fn process_checkpoint(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let [signer_info, authority_info, automation_info, board_info, miner_info, round_info, treasury_info, ticket_info, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    authority_info.is_writable()?;
    automation_info.has_seeds(&[AUTOMATION, &authority_info.key.to_bytes()], &grid_api::ID)?;
    let board = board_info
        .has_seeds(&[BOARD], &grid_api::ID)?
        .as_account::<Board>(&grid_api::ID)?;
    let miner = miner_info
        .is_writable()?
        .has_seeds(&[MINER, &authority_info.key.to_bytes()], &grid_api::ID)?
        .as_account_mut::<Miner>(&grid_api::ID)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    system_program.is_program(&system_program::ID)?;

    if miner.checkpoint_id == miner.round_id {
        return Ok(());
    }

    // Round closed after expiry: rewards are forfeited.
    round_info.has_seeds(&[ROUND, &miner.round_id.to_le_bytes()], &grid_api::ID)?;
    if round_info.data_is_empty() {
        miner.checkpoint_id = miner.round_id;
        return Ok(());
    }
    let round = round_info.as_account_mut::<Round>(&grid_api::ID)?;

    // Not reset yet.
    if round.id >= board.round_id {
        return Ok(());
    }

    // Expired: rewards are forfeited.
    if clock.slot >= round.expires_at {
        miner.checkpoint_id = miner.round_id;
        return Ok(());
    }

    let mut bot_fee = 0;
    if clock.slot >= round.expires_at.saturating_sub(TWELVE_HOURS_SLOTS) {
        bot_fee = std::mem::take(&mut miner.checkpoint_fee);
    }

    let mut rewards_sol = 0u64;
    let mut rewards_token = 0u64;
    let mut ticket_claimed = None;
    if round.void != 0 {
        rewards_sol = miner.deployed.iter().try_fold(0, |s, d| add(s, *d))?;
    } else {
        let w = round.winning_tile as usize;
        for (i, &d) in miner.deployed.iter().enumerate() {
            if d == 0 {
                continue;
            }
            let total = round.deployed[i];
            let (_, _, returned) = tile_split(total, i == w, round.admin_fee_bps, round.protocol_fee_bps)?;
            rewards_sol = add(rewards_sol, mul_div(d, returned, total)?)?;
        }

        let d = miner.deployed[w];
        if d > 0 {
            // SOL-weighted draws: this miner owns samples [cumulative, cumulative + d).
            let total = round.deployed[w];
            let (start, rng, id) = (miner.cumulative[w], round.rng, round.id);
            let lands = |tag: &[u8]| -> Result<bool, ProgramError> {
                let sample = draw(&rng, id, tag, total);
                Ok(sample >= start && sample < add(start, d)?)
            };
            if round.token_reward > 0 {
                if round.top_miner == SPLIT_ADDRESS {
                    rewards_token = mul_div(round.token_reward, d, total)?;
                } else if lands(b"solo")? {
                    rewards_token = round.token_reward;
                    round.top_miner = miner.authority;
                }
            }
            if round.pot_amount > 0 && lands(b"pot")? {
                round.pot_winner = miner.authority;
            }
            // Entries whose share rounds to 0 start claimed; no ticket if nothing is left.
            if round.vault_hit != 0 {
                let mut claimed = 0u64;
                for (i, &v) in round.vault.iter().enumerate() {
                    if mul_div(v, d, total)? == 0 {
                        claimed |= 1 << i;
                    }
                }
                if claimed != ALL_CLAIMED {
                    ticket_claimed = Some(claimed);
                }
            }
        }
    }

    // Credit `$GRID` (accounting only; claimed later with ClaimToken).
    miner.update_rewards(treasury);
    miner.checkpoint_id = round.id;
    miner.unrefined = add(miner.unrefined, rewards_token)?;
    miner.lifetime_token = add(miner.lifetime_token, rewards_token)?;
    miner.lifetime_sol = add(miner.lifetime_sol, rewards_sol)?;
    treasury.total_unrefined = add(treasury.total_unrefined, rewards_token)?;
    round.token_credited = add(round.token_credited, rewards_token)?;

    let automation = if !automation_info.data_is_empty() {
        let automation = automation_info
            .as_account_mut::<Automation>(&grid_api::ID)?
            .assert_mut_err(|a| a.authority == miner.authority, GridError::NotAuthorized.into())?;
        automation.total_token_earned = add(automation.total_token_earned, rewards_token)?;
        Some(automation)
    } else {
        None
    };

    // VaultTicket: the signer funds it, then is repaid out of this round's winnings (any
    // shortfall stays with the signer).
    if let Some(claimed) = ticket_claimed {
        create_program_account::<VaultTicket>(
            ticket_info,
            system_program,
            signer_info,
            &grid_api::ID,
            &[TICKET, &round.id.to_le_bytes(), &miner.authority.to_bytes()],
        )?;
        let ticket = ticket_info.as_account_mut::<VaultTicket>(&grid_api::ID)?;
        ticket.authority = miner.authority;
        ticket.round_id = round.id;
        ticket.share = miner.deployed[round.winning_tile as usize];
        ticket.claimed = claimed;
        let from_winnings = Rent::get()?.minimum_balance(VaultTicket::SIZE).min(rewards_sol);
        rewards_sol -= from_winnings;
        round_info.send(from_winnings, signer_info);
    }

    // Send SOL: automation (reload), else the authority, else (F18) hold it in the miner
    // when sending would leave an empty wallet below rent-exempt.
    if rewards_sol > 0 {
        match automation {
            Some(automation) if automation.reload > 0 => {
                automation.balance = add(automation.balance, rewards_sol)?;
                round_info.send(rewards_sol, automation_info);
            }
            _ => {
                let rent_floor = Rent::get()?.minimum_balance(0);
                if add(authority_info.lamports(), rewards_sol)? >= rent_floor {
                    round_info.send(rewards_sol, authority_info);
                } else {
                    miner.rewards_sol = add(miner.rewards_sol, rewards_sol)?;
                    round_info.send(rewards_sol, miner_info);
                }
            }
        }
    }
    if bot_fee > 0 {
        miner_info.send(bot_fee, signer_info);
    }

    let required = Rent::get()?.minimum_balance(Miner::SIZE);
    if miner_info.lamports() < add(add(required, miner.checkpoint_fee)?, miner.rewards_sol)? {
        return Err(GridError::InsufficientFunds.into());
    }
    Ok(())
}
