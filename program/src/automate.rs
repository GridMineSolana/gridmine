use grid_api::prelude::*;
use steel::*;

/// Creates, updates or (executor == default) closes the signer's automation.
pub fn process_automate(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<Automate>(data)?;
    let strategy = AutomationStrategy::parse(args.strategy)?;
    // Solo/split preferences only apply to the Random strategy.
    if (args.solo_tiles > 0 || args.split_tiles > 0) && strategy != AutomationStrategy::Random {
        return Err(GridError::InvalidStrategy.into());
    }
    if strategy == AutomationStrategy::DiscretionaryBps && args.fee > 100 {
        return Err(GridError::InvalidConfig.into());
    }

    let [signer_info, automation_info, executor_info, miner_info, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?;
    automation_info
        .is_writable()?
        .has_seeds(&[AUTOMATION, &signer_info.key.to_bytes()], &grid_api::ID)?;
    miner_info
        .is_writable()?
        .has_seeds(&[MINER, &signer_info.key.to_bytes()], &grid_api::ID)?;
    system_program.is_program(&system_program::ID)?;

    // Discretionary strategies need a named executor.
    if matches!(strategy, AutomationStrategy::Discretionary | AutomationStrategy::DiscretionaryBps)
        && *executor_info.key == EXECUTOR_ADDRESS
    {
        return Err(GridError::InvalidExecutor.into());
    }

    // Open the miner if needed.
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
        miner_info.as_account_mut::<Miner>(&grid_api::ID)?
    };

    // Close.
    if *executor_info.key == Pubkey::default() {
        automation_info.as_account::<Automation>(&grid_api::ID)?;
        automation_info.close(signer_info)?;
        return Ok(());
    }

    // Create or update.
    let automation = if automation_info.data_is_empty() {
        create_program_account::<Automation>(
            automation_info,
            system_program,
            signer_info,
            &grid_api::ID,
            &[AUTOMATION, &signer_info.key.to_bytes()],
        )?;
        let automation = automation_info.as_account_mut::<Automation>(&grid_api::ID)?;
        automation.authority = *signer_info.key;
        automation
    } else {
        automation_info.as_account_mut::<Automation>(&grid_api::ID)?
    };
    automation.amount = args.amount;
    automation.balance = add(automation.balance, args.deposit)?;
    automation.executor = *executor_info.key;
    automation.fee = args.fee;
    automation.mask = args.mask;
    automation.strategy = strategy as u64;
    automation.reload = (args.reload > 0) as u64;
    automation.solo_tiles = args.solo_tiles;
    automation.split_tiles = args.split_tiles;

    if miner.checkpoint_fee == 0 {
        miner.checkpoint_fee = CHECKPOINT_FEE;
        miner_info.collect(CHECKPOINT_FEE, signer_info)?;
    }
    automation_info.collect(args.deposit, signer_info)?;
    Ok(())
}
