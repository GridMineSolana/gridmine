use grid_api::prelude::*;
use solana_program::sysvar::slot_hashes;
use steel::*;

/// MagicBlock `RequestRandomnessScoped` instruction tag (first of 8 discriminator bytes).
const REQUEST_RANDOMNESS_SCOPED: u8 = 10;

/// Asks MagicBlock VRF for the current round's value once deposits are closed. Permissionless;
/// the signer pays the VRF fee (0.0005 SOL). May be repeated after `rerequest_after_slots`.
pub fn process_request_rng(accounts: &[AccountInfo<'_>], _data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let [signer_info, board_info, config_info, round_info, identity_info, queue_info, vrf_program, system_program, slot_hashes_info] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    signer_info.is_signer()?.is_writable()?;
    let board = board_info
        .has_seeds(&[BOARD], &grid_api::ID)?
        .as_account::<Board>(&grid_api::ID)?;
    let config = config_info
        .has_seeds(&[CONFIG], &grid_api::ID)?
        .as_account::<Config>(&grid_api::ID)?;
    let round = round_info
        .is_writable()?
        .has_seeds(&[ROUND, &board.round_id.to_le_bytes()], &grid_api::ID)?
        .as_account_mut::<Round>(&grid_api::ID)?;
    identity_info.has_seeds(&[IDENTITY], &grid_api::ID)?;
    queue_info.is_writable()?.has_address(&VRF_QUEUE)?;
    vrf_program.is_program(&VRF_PROGRAM_ID)?;
    system_program.is_program(&system_program::ID)?;
    slot_hashes_info.is_sysvar(&slot_hashes::ID)?;

    // end_slot is u64::MAX until the first deploy, so this also means the round has deposits.
    if clock.slot < board.end_slot {
        return Err(GridError::RoundNotOver.into());
    }
    let retry_slot = add(round.rng_requested_slot, config.params.rerequest_after_slots)?;
    if round.rng != [0; 32] || (round.rng_requested_slot != 0 && clock.slot < retry_slot) {
        return Err(GridError::RngRequested.into());
    }

    // Hand-rolled CPI: `ephemeral-vrf-sdk` 0.17.3 requires solana-program 3 (we are on 2.1).
    // Layout checked against magicblock-labs/solana-vrf @ 52b103c (request_randomness.rs).
    // Accounts: payer (w, s), our identity PDA [b"identity"] (s, via invoke_signed),
    //   queue (w), system program, SlotHashes.
    // Data: [10, 0 x 7] ‖ borsh RequestRandomness {
    //   caller_seed: [u8; 32], callback_program_id: Pubkey, callback_discriminator: Vec<u8>,
    //   callback_accounts_metas: Vec<{ pubkey: Pubkey, is_signer: bool, is_writable: bool }>,
    //   callback_args: Vec<u8> }   (borsh Vec = u32 LE length, then items)
    // MagicBlock then calls ConsumeRng with accounts [scoped vrf identity (s), round (w)] and data
    // [ConsumeRng] ‖ sha256(output) ‖ round_id_le. The callback metas hold the round only,
    // never our program id (solana-vrf issue #72).
    let mut data = vec![REQUEST_RANDOMNESS_SCOPED, 0, 0, 0, 0, 0, 0, 0];
    data.extend_from_slice(round_info.key.as_ref()); // caller_seed: the round PDA
    data.extend_from_slice(grid_api::ID.as_ref());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.push(GridInstruction::ConsumeRng as u8);
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(round_info.key.as_ref());
    data.extend_from_slice(&[0, 1]); // not signer, writable
    data.extend_from_slice(&8u32.to_le_bytes());
    data.extend_from_slice(&round.id.to_le_bytes());
    let ix = Instruction {
        program_id: VRF_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*signer_info.key, true),
            AccountMeta::new_readonly(*identity_info.key, true),
            AccountMeta::new(VRF_QUEUE, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(slot_hashes::ID, false),
        ],
        data,
    };
    invoke_signed(
        &ix,
        &[
            signer_info.clone(),
            identity_info.clone(),
            queue_info.clone(),
            system_program.clone(),
            slot_hashes_info.clone(),
        ],
        &grid_api::ID,
        &[IDENTITY],
    )?;
    round.rng_requested_slot = clock.slot;
    Ok(())
}

/// MagicBlock VRF callback. Stores the value once, only before `rng_deadline`; later callbacks
/// (re-requests, or past the void deadline) are ignored.
pub fn process_consume_rng(accounts: &[AccountInfo<'_>], data: &[u8]) -> ProgramResult {
    let clock = Clock::get()?;
    let args = parse_args::<ConsumeRng>(data)?;
    let [identity_info, round_info] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    identity_info.is_signer()?;
    if *identity_info.key != vrf_identity().0 {
        return Err(GridError::NotAuthorized.into());
    }
    let round = round_info
        .is_writable()?
        .as_account_mut::<Round>(&grid_api::ID)?
        .assert_mut_err(|r| r.id == args.round_id, GridError::InvalidRound.into())?;
    if round.rng == [0; 32] && clock.slot < round.rng_deadline {
        round.rng = args.rng;
    }
    Ok(())
}
