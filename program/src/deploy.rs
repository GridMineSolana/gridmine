use grid_api::prelude::*;
use solana_program::keccak::hashv;
use steel::*;

/// Deploys SOL on tiles of the current round (ORE, plus `!paused` and `min_deploy`).
pub fn process_deploy(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<Deploy>(data)?;
    let mut amount = args.amount;
    let clock = Clock::get()?;
    let [signer_info, authority_info, automation_info, board_info, config_info, miner_info, round_info, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    authority_info.is_writable()?;
    automation_info
        .is_writable()?
        .has_seeds(&[AUTOMATION, &authority_info.key.to_bytes()], &grid_api::ID)?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    if config.paused != 0 {
        return Err(GridError::Paused.into());
    }
    let board = board_info
        .has_seeds(&[BOARD], &grid_api::ID)?
        .as_account_mut::<Board>(&grid_api::ID)?
        .assert_mut_err(
            |b| clock.slot >= b.start_slot && clock.slot < b.end_slot,
            GridError::RoundNotActive.into(),
        )?;
    let round = round_info
        .has_seeds(&[ROUND, &board.round_id.to_le_bytes()], &grid_api::ID)?
        .as_account_mut::<Round>(&grid_api::ID)?;
    miner_info
        .is_writable()?
        .has_seeds(&[MINER, &authority_info.key.to_bytes()], &grid_api::ID)?;
    system_program.is_program(&system_program::ID)?;

    // Load the automation, if any.
    let automation = if !automation_info.data_is_empty() {
        Some(
            automation_info
                .as_account_mut::<Automation>(&grid_api::ID)?
                .assert_mut_err(
                    |a| a.executor == *signer_info.key || a.executor == EXECUTOR_ADDRESS,
                    GridError::InvalidExecutor.into(),
                )?
                .assert_mut_err(|a| a.authority == *authority_info.key, GridError::NotAuthorized.into())?,
        )
    } else if signer_info.key != authority_info.key {
        // Without an automation only the authority may deploy for its miner.
        return Err(GridError::NotAuthorized.into());
    } else {
        None
    };
    let strategy = match &automation {
        Some(a) => a.strategy,
        None => u64::MAX,
    };

    // Pick amount and tiles.
    let tiles = match &automation {
        None => mask_to_tiles(args.tiles),
        Some(automation) => match AutomationStrategy::parse(automation.strategy)? {
            AutomationStrategy::Preferred => {
                amount = automation.amount;
                mask_to_tiles(automation.mask)
            }
            AutomationStrategy::Random => {
                amount = automation.amount;
                random_tiles(automation, round)
            }
            AutomationStrategy::Discretionary | AutomationStrategy::DiscretionaryBps => {
                amount = amount.min(automation.amount);
                mask_to_tiles(args.tiles)
            }
        },
    };
    if amount < config.params.min_deploy {
        return Err(GridError::AmountTooSmall.into());
    }

    // Open or load the miner. Seeds use the signer, so an executor cannot create a miner.
    let miner = if miner_info.data_is_empty() {
        create_program_account::<Miner>(
            miner_info,
            system_program,
            signer_info,
            &grid_api::ID,
            &[MINER, &signer_info.key.to_bytes()],
        )?;
        let miner = miner_info.as_account_mut::<Miner>(&grid_api::ID)?;
        miner.authority = *signer_info.key;
        // No round yet. Starting at 0 would mark round-0 deposits as already checkpointed.
        miner.round_id = u64::MAX;
        miner.checkpoint_id = u64::MAX;
        miner
    } else {
        miner_info
            .as_account_mut::<Miner>(&grid_api::ID)?
            .assert_mut_err(|m| m.authority == *authority_info.key, GridError::NotAuthorized.into())?
    };

    // Move the miner to this round.
    if miner.round_id != round.id {
        if miner.checkpoint_id != miner.round_id {
            return Err(GridError::NotCheckpointed.into());
        }
        miner.deployed = [0; TILES];
        miner.cumulative = round.deployed;
        miner.round_id = round.id;
    }
    let is_first_deploy = miner.deployed.iter().all(|d| *d == 0);

    // F16: an automation that can't cover this deploy is closed, with no executor fee.
    if is_first_deploy {
        if let Some(automation) = &automation {
            let total = amount.saturating_mul(tiles.iter().filter(|t| **t).count() as u64);
            if automation.balance < total.saturating_add(automation.min_fee(total)) {
                automation_info.close(authority_info)?;
                return Ok(());
            }
        }
    }

    // Record deploys. One deploy per tile per round per miner (ORE).
    let mut total_amount = 0u64;
    let mut total_tiles = 0u64;
    let mut deployed_mask = 0u64;
    for (tile, _) in tiles.iter().enumerate().filter(|(_, t)| **t) {
        if miner.deployed[tile] > 0 {
            continue;
        }
        miner.cumulative[tile] = round.deployed[tile];
        miner.deployed[tile] = amount;
        round.deployed[tile] = add(round.deployed[tile], amount)?;
        round.count[tile] += 1;
        total_amount = add(total_amount, amount)?;
        total_tiles += 1;
        deployed_mask |= 1 << tile;
    }
    if is_first_deploy && total_amount > 0 {
        round.total_miners += 1;
    }
    // The first deposit starts the round timer.
    if board.end_slot == u64::MAX && total_amount > 0 {
        board.start_slot = clock.slot;
        board.end_slot = add(clock.slot, config.params.round_slots)?;
        round.rng_deadline = add(board.end_slot, config.params.void_after_slots)?;
    }
    miner.lifetime_deployed = add(miner.lifetime_deployed, total_amount)?;

    // Hold back the checkpoint bot fee once.
    if miner.checkpoint_fee == 0 {
        miner.checkpoint_fee = CHECKPOINT_FEE;
        miner_info.collect(CHECKPOINT_FEE, signer_info)?;
    }

    // Move SOL into the round.
    if let Some(automation) = automation {
        let fee = if is_first_deploy && total_amount > 0 { automation.min_fee(total_amount) } else { 0 };
        automation.total_sol_spent = add(automation.total_sol_spent, total_amount)?;
        automation.balance = sub(automation.balance, add(total_amount, fee)?)?;
        automation_info.send(total_amount, round_info);
        automation_info.send(fee, signer_info);
        // Close once the balance can't cover one more tile.
        if automation.balance < automation.amount.saturating_add(automation.min_fee(automation.amount)) {
            automation_info.close(authority_info)?;
        }
    } else {
        round_info.collect(total_amount, signer_info)?;
    }

    DeployEvent {
        disc: GridEvent::Deploy as u64,
        authority: miner.authority,
        amount,
        mask: deployed_mask,
        round_id: round.id,
        signer: *signer_info.key,
        strategy,
        total_tiles,
        ts: clock.unix_timestamp,
    }
    .log();
    Ok(())
}

fn mask_to_tiles(mask: u64) -> [bool; TILES] {
    core::array::from_fn(|i| mask & (1 << i) != 0)
}

/// ORE's Random strategy: first deploy uses the mask; later rounds pick tiles
/// deterministically from (authority, round id), optionally by solo/split preference.
fn random_tiles(automation: &Automation, round: &Round) -> [bool; TILES] {
    if automation.total_sol_spent == 0 {
        return mask_to_tiles(automation.mask);
    }
    if automation.solo_tiles == 0 && automation.split_tiles == 0 {
        let n = automation.mask.count_ones() as u64;
        let r = hashv(&[&automation.authority.to_bytes(), &round.id.to_le_bytes()]).0;
        return generate_random_mask(n, &r);
    }
    let distribution_mask = round.distribution_mask();
    let mut solo = [0usize; TILES];
    let mut split = [0usize; TILES];
    let (mut num_solo, mut num_split) = (0, 0);
    for i in 0..TILES {
        if distribution_mask & (1 << i) != 0 {
            solo[num_solo] = i;
            num_solo += 1;
        } else {
            split[num_split] = i;
            num_split += 1;
        }
    }
    let seed = hashv(&[
        &automation.authority.to_bytes(),
        &round.id.to_le_bytes(),
        b"solo_split_mask",
    ])
    .0;
    deterministic_shuffle(&mut solo[..num_solo], &seed);
    deterministic_shuffle(&mut split[..num_split], &seed);
    let mut tiles = [false; TILES];
    for &i in &solo[..num_solo.min(automation.solo_tiles as usize)] {
        tiles[i] = true;
    }
    for &i in &split[..num_split.min(automation.split_tiles as usize)] {
        tiles[i] = true;
    }
    tiles
}

fn deterministic_shuffle(idxs: &mut [usize], seed: &[u8; 32]) {
    for i in (1..idxs.len()).rev() {
        let start = i % (32 - 8 + 1);
        let r = u64::from_le_bytes(seed[start..start + 8].try_into().unwrap());
        idxs.swap(i, (r % (i as u64 + 1)) as usize);
    }
}

fn generate_random_mask(num_tiles: u64, r: &[u8; 32]) -> [bool; TILES] {
    let mut mask = [false; TILES];
    let mut selected = 0u64;
    for (i, slot) in mask.iter_mut().enumerate() {
        let remaining_needed = num_tiles.saturating_sub(selected);
        let remaining_positions = (TILES - i) as u64;
        if remaining_needed > 0 && (r[i] as u64) * remaining_positions < remaining_needed * 256 {
            *slot = true;
            selected += 1;
        }
    }
    mask
}
