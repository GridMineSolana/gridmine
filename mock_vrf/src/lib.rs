//! Test-only stand-in for MagicBlock VRF, loaded at its program id in LiteSVM.
//! - Tag 10 (RequestRandomnessScoped): the caller's `[b"identity"]` PDA must sign. Takes the
//!   0.0005 SOL fee into the queue and stores the borsh request (u32 LE length + bytes) in the
//!   queue account's data.
//! - Tag 4 (provide), data `[4] ‖ value[32]`, accounts `[scoped identity, queue, callback
//!   program, ...callback metas]`: replays the stored request as a callback signed by the scoped
//!   identity `[b"identity", callback_program]`, like `provide_randomness.rs`.
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_instruction,
};

entrypoint!(process);

fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    match data.first() {
        Some(10) => request(accounts, &data[8..]),
        Some(4) => provide(program_id, accounts, &data[1..33]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn request(accounts: &[AccountInfo], payload: &[u8]) -> ProgramResult {
    let [payer, identity, queue, system, _slot_hashes] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let callback_program = Pubkey::try_from(&payload[32..64]).unwrap();
    let caller_identity = Pubkey::find_program_address(&[b"identity"], &callback_program).0;
    if !payer.is_signer || !identity.is_signer || *identity.key != caller_identity {
        return Err(ProgramError::MissingRequiredSignature);
    }
    invoke(
        &system_instruction::transfer(payer.key, queue.key, 500_000),
        &[payer.clone(), queue.clone(), system.clone()],
    )?;
    let mut q = queue.try_borrow_mut_data()?;
    q[..4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    q[4..4 + payload.len()].copy_from_slice(payload);
    Ok(())
}

fn provide(program_id: &Pubkey, accounts: &[AccountInfo], value: &[u8]) -> ProgramResult {
    let [identity, queue, callback, metas_infos @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let q = queue.try_borrow_data()?.to_vec();
    let len = u32::from_le_bytes(q[..4].try_into().unwrap()) as usize;
    let mut r = &q[4..4 + len];
    take(&mut r, 32); // caller_seed
    let callback_program = Pubkey::try_from(take(&mut r, 32)).unwrap();
    let disc = take_vec(&mut r, 1).to_vec();
    let metas = take_vec(&mut r, 34).chunks(34).map(|m| AccountMeta {
        pubkey: Pubkey::try_from(&m[..32]).unwrap(),
        is_signer: m[32] == 1,
        is_writable: m[33] == 1,
    });
    let args = take_vec(&mut r, 1);

    let (scoped, bump) = Pubkey::find_program_address(&[b"identity", callback_program.as_ref()], program_id);
    if *identity.key != scoped || *callback.key != callback_program {
        return Err(ProgramError::InvalidArgument);
    }
    let mut accounts = vec![AccountMeta::new_readonly(scoped, true)];
    accounts.extend(metas);
    let data = [disc.as_slice(), value, args].concat();
    let mut infos = vec![identity.clone()];
    infos.extend_from_slice(metas_infos);
    invoke_signed(
        &Instruction { program_id: callback_program, accounts, data },
        &infos,
        &[&[b"identity", callback_program.as_ref(), &[bump]]],
    )
}

fn take<'a>(r: &mut &'a [u8], n: usize) -> &'a [u8] {
    let (head, tail) = r.split_at(n);
    *r = tail;
    head
}

/// Borsh `Vec<T>` with `item`-byte items.
fn take_vec<'a>(r: &mut &'a [u8], item: usize) -> &'a [u8] {
    let n = u32::from_le_bytes(take(r, 4).try_into().unwrap()) as usize;
    take(r, n * item)
}
