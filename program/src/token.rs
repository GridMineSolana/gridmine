use grid_api::prelude::*;
use spl_token_2022::{
    extension::{
        default_account_state::DefaultAccountState, non_transferable::NonTransferable,
        transfer_fee::TransferFeeConfig, transfer_hook::TransferHook, BaseStateWithExtensions,
        StateWithExtensions,
    },
    state::{AccountState, Mint},
};
use steel::*;

/// Reads a SPL Token or Token-2022 mint (with extensions). Returns (decimals, is_t22).
/// Rejects TransferFee, a set TransferHook program, NonTransferable and DefaultAccountState=Frozen.
/// PermanentDelegate is allowed because xStocks use it. Custody risk: the issuer can move or burn
/// tokens in our Treasury ATAs, so pot/vault books for such a tile can exceed the real balance.
pub fn read_tile_mint(mint_info: &AccountInfo) -> Result<(u8, bool), ProgramError> {
    let is_t22 = match *mint_info.owner {
        spl_token::ID => false,
        spl_token_2022::ID => true,
        _ => return Err(GridError::UnsupportedMint.into()),
    };
    let data = mint_info.try_borrow_data()?;
    let mint = StateWithExtensions::<Mint>::unpack(&data)
        .map_err(|_| ProgramError::from(GridError::UnsupportedMint))?;
    if !mint.base.is_initialized
        || mint.get_extension::<TransferFeeConfig>().is_ok()
        || mint.get_extension::<NonTransferable>().is_ok()
        || mint
            .get_extension::<DefaultAccountState>()
            .is_ok_and(|d| d.state == AccountState::Frozen as u8)
        || mint
            .get_extension::<TransferHook>()
            .is_ok_and(|hook| Option::<Pubkey>::from(hook.program_id).is_some())
    {
        return Err(GridError::UnsupportedMint.into());
    }
    Ok((mint.base.decimals, is_t22))
}

/// Creates `owner`'s ATA if it does not exist; otherwise checks it is that ATA.
pub fn ensure_ata<'info>(
    payer: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    ata_info: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    ata_program: &AccountInfo<'info>,
) -> ProgramResult {
    if ata_info.data_is_empty() {
        create_associated_token_account(
            payer,
            owner,
            ata_info,
            mint,
            system_program,
            token_program,
            ata_program,
        )
    } else {
        ata_info.as_associated_token_account(owner.key, mint.key)?;
        Ok(())
    }
}

/// Token-2022-aware `transfer_checked` out of a Treasury-owned token account. Skips zero.
pub fn send_from_treasury<'info>(
    treasury: &AccountInfo<'info>,
    from: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    decimals: u8,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }
    transfer_checked_signed(treasury, from, mint, to, token_program, amount, decimals, &[TREASURY])
}
