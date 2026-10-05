use grid_api::prelude::*;
use solana_program::program::invoke;
use steel::*;

/// Worker gate for BuyAsset and Buyback: signer is `config.worker`, `0 < amount_in <=
/// max_swap_lamports`, and `min_swap_interval_slots` passed since the last swap.
pub fn check_worker(signer_info: &AccountInfo<'_>, config: &mut Config, amount_in: u64) -> ProgramResult {
    signer_info.is_signer()?;
    if *signer_info.key != config.worker {
        return Err(GridError::NotAuthorized.into());
    }
    let slot = Clock::get()?.slot;
    let p = &config.params;
    if amount_in == 0
        || amount_in > p.max_swap_lamports
        || slot < add(config.last_swap_slot, p.min_swap_interval_slots)?
    {
        return Err(GridError::SwapLimit.into());
    }
    config.last_swap_slot = slot;
    Ok(())
}

pub struct SwapAccounts<'a, 'info> {
    pub treasury: &'a AccountInfo<'info>,
    pub swap: &'a AccountInfo<'info>,
    pub swap_wsol: &'a AccountInfo<'info>,
    /// Treasury's ATA for the output mint.
    pub dest: &'a AccountInfo<'info>,
    pub wsol_program: &'a AccountInfo<'info>,
    pub jupiter: &'a AccountInfo<'info>,
    /// Jupiter's route accounts, passed through.
    pub route: &'a [AccountInfo<'info>],
}

/// PLAN §6.2 swap helper (security core). Moves `lamports` of Treasury SOL plus `wsol` from the
/// Treasury's wSOL ATA (Buyback's creator fees) into the swap PDA's wSOL ATA, syncs it, and CPIs
/// Jupiter with only the swap PDA signing. Afterwards the swap ATA has spent all of it, the
/// Treasury's lamports moved only by `lamports`, and `dest` rose by at least `min_out`.
/// Returns the amount received. The caller has already debited its bucket.
#[allow(clippy::too_many_arguments)]
pub fn swap<'info>(
    a: &SwapAccounts<'_, 'info>,
    out_mint: &Pubkey,
    out_program: &Pubkey,
    lamports: u64,
    wsol: Option<(&AccountInfo<'info>, u64)>,
    min_out: u64,
    route_data: &[u8],
) -> Result<u64, ProgramError> {
    let treasury = a.treasury.key;
    a.swap.has_seeds(&[SWAP], &grid_api::ID)?;
    a.swap_wsol.is_writable()?.has_owner(&spl_token::ID)?;
    swap_ata_amount(a.swap_wsol, a.swap.key)?;
    a.dest.is_writable()?.has_owner(out_program)?;
    let dest_before = treasury_ata_amount(a.dest, treasury, out_mint)?;
    a.wsol_program.is_program(&spl_token::ID)?;
    a.jupiter.is_program(&JUPITER_PROGRAM_ID)?;
    if min_out == 0 {
        return Err(GridError::MinOut.into());
    }

    // The route may not see any GRID account (no reentry into our books) nor any Treasury
    // token account other than `dest`. Only the swap PDA signs, so nothing else of ours can move.
    for info in a.route {
        let is_token = *info.owner == spl_token::ID || *info.owner == spl_token_2022::ID;
        let treasury_owned = is_token && info.data.borrow().get(32..64) == Some(treasury.as_ref());
        if info.key == treasury || *info.owner == grid_api::ID || (treasury_owned && info.key != a.dest.key) {
            return Err(GridError::BadRoute.into());
        }
    }

    // Fund the swap PDA's wSOL ATA: wSOL first (its CPI must not see unsynced lamports), then
    // lamports, then sync_native.
    let treasury_lamports = a.treasury.lamports();
    let mut amount_in = lamports;
    if let Some((treasury_wsol, amount)) = wsol.filter(|w| w.1 > 0) {
        transfer_signed(a.treasury, treasury_wsol, a.swap_wsol, a.wsol_program, amount, &[TREASURY])?;
        amount_in = add(amount_in, amount)?;
    }
    a.treasury.send(lamports, a.swap_wsol);
    // The next CPI must carry both sides of the lamport move (runtime balance check). A no-op Revoke
    // ignores extra accounts in spl-token and p-token; sync_native's 2nd account is Rent in p-token.
    let mut revoke = spl_token::instruction::revoke(&spl_token::ID, a.swap_wsol.key, a.swap.key, &[])?;
    revoke.accounts.push(AccountMeta::new(*treasury, false));
    let revoke_infos = [a.swap_wsol.clone(), a.swap.clone(), a.treasury.clone(), a.wsol_program.clone()];
    invoke_signed(&revoke, &revoke_infos, &grid_api::ID, &[SWAP])?;
    invoke(&spl_token::instruction::sync_native(&spl_token::ID, a.swap_wsol.key)?, &[a.swap_wsol.clone(), a.wsol_program.clone()])?;
    let wsol_before = swap_ata_amount(a.swap_wsol, a.swap.key)?;

    // Jupiter CPI: accounts and data passed through, only the swap PDA signs.
    let metas = a
        .route
        .iter()
        .map(|info| AccountMeta { pubkey: *info.key, is_signer: info.key == a.swap.key, is_writable: info.is_writable })
        .collect();
    let mut infos = a.route.to_vec();
    infos.push(a.jupiter.clone());
    invoke_signed(
        &Instruction { program_id: JUPITER_PROGRAM_ID, accounts: metas, data: route_data.to_vec() },
        &infos,
        &grid_api::ID,
        &[SWAP],
    )?;

    // Post-swap checks.
    if a.treasury.lamports() != sub(treasury_lamports, lamports)? {
        return Err(GridError::BadRoute.into());
    }
    if swap_ata_amount(a.swap_wsol, a.swap.key)? > sub(wsol_before, amount_in)? {
        return Err(GridError::BadRoute.into());
    }
    let received = treasury_ata_amount(a.dest, treasury, out_mint)?
        .checked_sub(dest_before)
        .ok_or(GridError::BadRoute)?;
    if received < min_out {
        return Err(GridError::MinOut.into());
    }
    Ok(received)
}

/// Balance of the swap PDA's wSOL ATA. Fails if the route closed it or handed out its authority.
fn swap_ata_amount(info: &AccountInfo<'_>, swap: &Pubkey) -> Result<u64, ProgramError> {
    let account = info.as_associated_token_account(swap, &SOL_MINT)?;
    if account.owner() != *swap || account.delegate().is_some() || account.close_authority().is_some() {
        return Err(GridError::BadRoute.into());
    }
    Ok(account.amount())
}

/// Balance of the Treasury's ATA for `mint`.
pub fn treasury_ata_amount(info: &AccountInfo<'_>, treasury: &Pubkey, mint: &Pubkey) -> Result<u64, ProgramError> {
    let account = info.as_associated_token_account(treasury, mint)?;
    if account.owner() != *treasury || account.mint() != *mint {
        return Err(GridError::BadRoute.into());
    }
    Ok(account.amount())
}
