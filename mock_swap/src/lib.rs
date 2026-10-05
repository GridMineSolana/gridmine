//! Test-only stand-in for Jupiter v6, loaded at its program id in LiteSVM.
//! Data: `[mode] ‖ in_amount u64 ‖ out_amount u64`. Accounts: `[swap PDA (s), swap wSOL ATA,
//! sink wSOL account, source out-token account, destination, source authority PDA [b"src"],
//! wSOL token program, out token program, ...extra]`.
//! Takes `in_amount` wSOL from the swap ATA (signed by the swap PDA), then pays out of `source`:
//! - 0 honest: `out_amount`;
//! - 1 short: `out_amount / 2`;
//! - 2 steals: honest, plus moves `out_amount` from `extra[0]` (another Treasury ATA) to `source`.
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

entrypoint!(process);

fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [swap, swap_wsol, sink, source, dest, src_authority, wsol_program, out_program, extra @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if data.len() != 17 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let in_amount = u64::from_le_bytes(data[1..9].try_into().unwrap());
    let out_amount = u64::from_le_bytes(data[9..17].try_into().unwrap());
    invoke(&transfer(wsol_program.key, swap_wsol.key, sink.key, swap.key, in_amount), &[swap_wsol.clone(), sink.clone(), swap.clone()])?;
    let out = if data[0] == 1 { out_amount / 2 } else { out_amount };
    let (_, bump) = Pubkey::find_program_address(&[b"src"], program_id);
    invoke_signed(
        &transfer(out_program.key, source.key, dest.key, src_authority.key, out),
        &[source.clone(), dest.clone(), src_authority.clone()],
        &[&[b"src", &[bump]]],
    )?;
    if data[0] == 2 {
        let victim = extra.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
        invoke(&transfer(out_program.key, victim.key, source.key, swap.key, out_amount), &[victim.clone(), source.clone(), swap.clone()])?;
    }
    Ok(())
}

/// SPL Token / Token-2022 `Transfer` (tag 3).
fn transfer(program: &Pubkey, from: &Pubkey, to: &Pubkey, authority: &Pubkey, amount: u64) -> Instruction {
    let mut data = vec![3];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: *program,
        accounts: vec![AccountMeta::new(*from, false), AccountMeta::new(*to, false), AccountMeta::new_readonly(*authority, true)],
        data,
    }
}
