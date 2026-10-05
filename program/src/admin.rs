use grid_api::prelude::*;
use steel::*;

use crate::token::{ensure_ata, read_tile_mint};

/// Creates Config, Board, Round 0, Treasury and the wSOL ATAs (Treasury + swap PDA). Starts paused.
pub fn process_initialize(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<Initialize>(data)?;
    let [signer_info, config_info, board_info, round_info, treasury_info, treasury_wsol_info, swap_info, swap_wsol_info, wsol_mint_info, system_program, token_program, ata_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?.has_address(&INIT_AUTHORITY)?;
    config_info.is_empty()?.is_writable()?.has_seeds(&[CONFIG], &grid_api::ID)?;
    board_info.is_empty()?.is_writable()?.has_seeds(&[BOARD], &grid_api::ID)?;
    round_info.is_empty()?.is_writable()?.has_seeds(&[ROUND, &0u64.to_le_bytes()], &grid_api::ID)?;
    treasury_info.is_empty()?.is_writable()?.has_seeds(&[TREASURY], &grid_api::ID)?;
    swap_info.has_seeds(&[SWAP], &grid_api::ID)?;
    wsol_mint_info.has_address(&SOL_MINT)?;
    system_program.is_program(&system_program::ID)?;
    token_program.is_program(&spl_token::ID)?;
    ata_program.is_program(&spl_associated_token_account::ID)?;
    args.params.validate()?;
    if (args.token_program != spl_token::ID && args.token_program != spl_token_2022::ID)
        || [args.worker, args.admin_collector, args.team_collector, args.token_mint]
            .contains(&Pubkey::default())
    {
        return Err(GridError::InvalidConfig.into());
    }

    create_program_account::<Config>(config_info, system_program, signer_info, &grid_api::ID, &[CONFIG])?;
    let config = config_info.as_account_mut::<Config>(&grid_api::ID)?;
    config.admin = *signer_info.key;
    config.worker = args.worker;
    config.admin_collector = args.admin_collector;
    config.team_collector = args.team_collector;
    config.token_mint = args.token_mint;
    config.token_program = args.token_program;
    config.token_decimals = args.token_decimals;
    config.paused = 1;
    config.params = args.params;

    create_program_account::<Board>(board_info, system_program, signer_info, &grid_api::ID, &[BOARD])?;
    let board = board_info.as_account_mut::<Board>(&grid_api::ID)?;
    board.end_slot = u64::MAX;

    create_program_account::<Round>(
        round_info,
        system_program,
        signer_info,
        &grid_api::ID,
        &[ROUND, &0u64.to_le_bytes()],
    )?;
    let round = round_info.as_account_mut::<Round>(&grid_api::ID)?;
    round.expires_at = u64::MAX;
    round.winning_tile = u64::MAX;
    round.rent_payer = *signer_info.key;

    create_program_account::<Treasury>(treasury_info, system_program, signer_info, &grid_api::ID, &[TREASURY])?;

    // wSOL ATAs: the Treasury's receives PumpSwap creator fees; the swap PDA's feeds Jupiter.
    // Anyone can create them first, so an existing one is checked, not recreated.
    for (owner, wsol_ata) in [(treasury_info, treasury_wsol_info), (swap_info, swap_wsol_info)] {
        ensure_ata(
            signer_info,
            owner,
            wsol_ata,
            wsol_mint_info,
            system_program,
            token_program,
            ata_program,
        )?;
    }
    Ok(())
}

pub fn process_update_config(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<UpdateConfig>(data)?;
    let [signer_info, config_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = load_config_as_admin(signer_info, config_info)?;
    args.params.validate()?;
    if [args.admin, args.worker, args.admin_collector, args.team_collector].contains(&Pubkey::default()) {
        return Err(GridError::InvalidConfig.into());
    }
    config.admin = args.admin;
    config.worker = args.worker;
    config.admin_collector = args.admin_collector;
    config.team_collector = args.team_collector;
    config.params = args.params;
    Ok(())
}

/// Sets tile `index` to `mint`. Only while that tile holds no pot tokens, no vault balance, and no
/// unclosed round still owes its tokens (`owed`).
/// Each mint has one Treasury ATA, so a mint can't back two tiles, `$GRID` or wSOL.
pub fn process_set_tile(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<SetTile>(data)?;
    let [signer_info, config_info, treasury_info, mint_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = load_config_as_admin(signer_info, config_info)?;
    let treasury = treasury_info
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account::<Treasury>(&grid_api::ID)?;
    let index = usize::try_from(args.index).map_err(|_| ProgramError::from(GridError::InvalidTile))?;
    if index >= TILES {
        return Err(GridError::InvalidTile.into());
    }
    if treasury.pot_tokens[index] != 0 || treasury.vault[index] != 0 || treasury.owed[index] != 0 {
        return Err(GridError::TileNotEmpty.into());
    }
    let mint = *mint_info.key;
    if mint == config.token_mint
        || mint == SOL_MINT
        || config.tiles.iter().enumerate().any(|(i, t)| i != index && t.mint == mint)
    {
        return Err(GridError::UnsupportedMint.into());
    }
    let (decimals, is_t22) = read_tile_mint(mint_info)?;
    config.tiles[index] = Tile { mint, is_t22: is_t22 as u8, decimals, _pad: [0; 6] };
    Ok(())
}

/// Blocks Deploy only. Reset, Checkpoint and claims keep working.
pub fn process_set_pause(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<SetPause>(data)?;
    let [signer_info, config_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if args.paused > 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    load_config_as_admin(signer_info, config_info)?.paused = args.paused;
    Ok(())
}

/// Pays out of the `admin_sol` (bucket 0) or `team_sol` (bucket 1) Treasury bucket to its collector.
pub fn process_withdraw_team(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let args = parse_args::<WithdrawTeam>(data)?;
    let [signer_info, config_info, treasury_info, collector_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let config = load_config_as_admin(signer_info, config_info)?;
    let treasury = treasury_info
        .is_writable()?
        .has_seeds(&[TREASURY], &grid_api::ID)?
        .as_account_mut::<Treasury>(&grid_api::ID)?;
    let (bucket, collector) = match args.bucket {
        0 => (&mut treasury.admin_sol, config.admin_collector),
        1 => (&mut treasury.team_sol, config.team_collector),
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    collector_info.is_writable()?.has_address(&collector)?;
    *bucket = bucket.checked_sub(args.amount).ok_or(GridError::InsufficientFunds)?;
    treasury_info.send(args.amount, collector_info);
    Ok(())
}

fn load_config_as_admin<'a>(
    signer_info: &AccountInfo<'_>,
    config_info: &'a AccountInfo<'_>,
) -> Result<&'a mut Config, ProgramError> {
    signer_info.is_signer()?;
    config_info
        .is_writable()?
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account_mut::<Config>(&grid_api::ID)?
        .assert_mut_err(|c| c.admin == *signer_info.key, GridError::NotAuthorized.into())
}
