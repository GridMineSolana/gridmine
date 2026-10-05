use grid_api::prelude::*;
use steel::*;

/// Settles the current round (or voids it) and opens the next one. Permissionless.
pub fn process_reset(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let [signer_info, board_info, config_info, round_info, round_next_info, treasury_info, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    let params = config.params;
    let board = board_info
        .has_seeds(&[BOARD], &grid_api::ID)?
        .as_account_mut::<Board>(&grid_api::ID)?;
    // F22: explicit check instead of relying on u64::MAX + intermission overflowing.
    if board.end_slot == u64::MAX || clock.slot < add(board.end_slot, params.intermission_slots)? {
        return Err(GridError::RoundNotOver.into());
    }
    let round = round_info
        .is_writable()?
        .has_seeds(&[ROUND, &board.round_id.to_le_bytes()], &grid_api::ID)?
        .as_account_mut::<Round>(&grid_api::ID)?;
    let next_id = add(board.round_id, 1)?;
    round_next_info
        .is_empty()?
        .is_writable()?
        .has_seeds(&[ROUND, &next_id.to_le_bytes()], &grid_api::ID)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    system_program.is_program(&system_program::ID)?;

    let total_deployed = round.total_deployed();
    let mut event = ResetEvent {
        disc: GridEvent::Reset as u64,
        round_id: round.id,
        start_slot: board.start_slot,
        end_slot: board.end_slot,
        winning_tile: u64::MAX,
        total_miners: round.total_miners,
        total_deployed,
        ts: clock.unix_timestamp,
        ..Default::default()
    };

    // Claim window starts now, so a late Reset never eats into it (hit rounds: 30 days, below).
    round.expires_at = add(clock.slot, ONE_DAY_SLOTS)?;
    if round.rng != [0; 32] {
        let w = draw(&round.rng, round.id, b"tile", TILES as u64) as usize;
        round.winning_tile = w as u64;
        round.admin_fee_bps = params.admin_fee_bps;
        round.protocol_fee_bps = params.protocol_fee_bps;

        // Fees: admin on every tile, protocol on losing tiles.
        let (mut admin_fee, mut protocol_fee) = (0, 0);
        for (i, &d) in round.deployed.iter().enumerate() {
            let (a, p, _) = tile_split(d, i == w, params.admin_fee_bps, params.protocol_fee_bps)?;
            admin_fee = add(admin_fee, a)?;
            protocol_fee = add(protocol_fee, p)?;
        }
        round.admin_fee = admin_fee;
        round.protocol_fee = protocol_fee;
        round.total_returned_sol = sub(sub(total_deployed, admin_fee)?, protocol_fee)?;

        // Protocol fee into Treasury buckets.
        let b = bucket_split(
            protocol_fee,
            params.burn_bps,
            params.recycle_bps,
            params.pots_bps,
            params.vault_bps,
        )?;
        // The admin fee stays in the Treasury until WithdrawTeam, so a bad collector can't block Reset.
        treasury.admin_sol = add(treasury.admin_sol, admin_fee)?;
        treasury.buyback_sol = add(treasury.buyback_sol, b.buyback)?;
        treasury.vault_sol = add(treasury.vault_sol, b.vault)?;
        treasury.team_sol = add(treasury.team_sol, b.team)?;
        for pot in treasury.pot_sol.iter_mut() {
            *pot = add(*pot, b.pot_per_tile)?;
        }

        // F1: emit only when the winning tile has SOL on it.
        if round.deployed[w] > 0 {
            let boost = if clock.unix_timestamp < params.boost_until { params.boost_bps } else { DENOMINATOR_BPS };
            let e = emission(treasury.reserve, params.emission_ppb, boost, params.max_emission);
            let vault_part = mul_div(e, params.vault_emission_bps, DENOMINATOR_BPS)?;
            treasury.reserve = sub(treasury.reserve, e)?;
            treasury.vault[VAULT_GRID] = add(treasury.vault[VAULT_GRID], vault_part)?;
            round.token_reward = sub(e, vault_part)?;
            event.emission = e;
        }
        if round.deployed[w] > 0 {
            // Pot: the winning tile's swapped tokens go to one winner, picked at Checkpoint.
            round.pot_amount = std::mem::take(&mut treasury.pot_tokens[w]);
            treasury.owed[w] = add(treasury.owed[w], round.pot_amount)?;
            // Vault: hits 1 in vault_odds. The round takes the whole Vault and lives 30 days.
            if draw(&round.rng, round.id, b"vault", params.vault_odds) == 0 {
                round.vault_hit = 1;
                round.vault = std::mem::take(&mut treasury.vault);
                for (owed, amount) in treasury.owed.iter_mut().zip(round.vault) {
                    *owed = add(*owed, amount)?;
                }
                round.expires_at = add(clock.slot, VAULT_EXPIRY_SLOTS)?;
            }
        }
        if round.is_split_reward(w) {
            round.top_miner = SPLIT_ADDRESS;
        }

        round_info.send(add(admin_fee, protocol_fee)?, treasury_info);

        event.winning_tile = w as u64;
        event.top_miner = round.top_miner;
        event.admin_fee = admin_fee;
        event.protocol_fee = protocol_fee;
        event.total_returned = round.total_returned_sol;
        event.token_reward = round.token_reward;
        event.deployed_winning_tile = round.deployed[w];
    } else if clock.slot >= round.rng_deadline {
        // Void: no randomness in time. No fees, no emission; Checkpoint refunds 100%.
        round.void = 1;
        round.total_returned_sol = total_deployed;
        event.total_returned = total_deployed;
    } else {
        return Err(GridError::RngNotReady.into());
    }
    event.log();

    // Open the next round. F21: pass the system program.
    create_program_account::<Round>(
        round_next_info,
        system_program,
        signer_info,
        &grid_api::ID,
        &[ROUND, &next_id.to_le_bytes()],
    )?;
    let round_next = round_next_info.as_account_mut::<Round>(&grid_api::ID)?;
    round_next.id = next_id;
    round_next.expires_at = u64::MAX;
    round_next.winning_tile = u64::MAX;
    round_next.rent_payer = *signer_info.key;

    board.round_id = next_id;
    board.start_slot = clock.slot + 1;
    board.end_slot = u64::MAX;
    Ok(())
}
