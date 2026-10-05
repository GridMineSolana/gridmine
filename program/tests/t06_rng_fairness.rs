//! T6: only MagicBlock's scoped identity for this program can deliver the value, only for the
//! matching round, and only once. RequestRng waits for deposits to close and respects the retry gap.
mod common;

use common::*;
use grid_api::prelude::*;
use solana_sdk::pubkey::Pubkey;

fn setup() -> (Game, u64) {
    let mut game = Game::ready();
    let miner = game.user();
    game.deploy(miner, 10_000_000, tile(0)).unwrap();
    let id = game.board().round_id;
    (game, id)
}

#[test]
fn t6_consume_rng_checks_signer_round_and_overwrite() {
    let (mut game, id) = setup();
    game.warp_to(game.board().end_slot);
    game.request_rng(ADMIN).unwrap();

    // Not the VRF identity.
    let fake = game.user();
    let res = game.send(&[consume_rng(fake, id, [9; 32])], fake);
    assert_eq!(error_code(res), GridError::NotAuthorized as u32);

    // Right identity (sigverify is off, so the test can sign as it), wrong round id.
    let mut ix = consume_rng(vrf_identity().0, id, [9; 32]);
    ix.data = ConsumeRng { rng: [9; 32], round_id: id + 1 }.to_bytes();
    assert_eq!(error_code(game.send(&[ix], ADMIN)), GridError::InvalidRound as u32);
    assert_eq!(game.round(id).rng, [0; 32]);

    // The first answer is stored; a second one (e.g. from a re-request) can't overwrite it.
    game.vrf_answer(id, [1; 32]).unwrap();
    game.vrf_answer(id, [2; 32]).unwrap();
    assert_eq!(game.round(id).rng, [1; 32]);
}

#[test]
fn t6_request_rng_gating() {
    let (mut game, id) = setup();
    assert_eq!(error_code(game.request_rng(ADMIN)), GridError::RoundNotOver as u32);

    let end = game.board().end_slot;
    game.warp_to(end);
    game.request_rng(ADMIN).unwrap();
    assert_eq!(game.round(id).rng_requested_slot, end);

    // Callback metas are the round only (never our program id); args are the round id.
    let q = game.svm.get_account(&VRF_QUEUE).unwrap().data;
    let payload = &q[4..4 + u32::from_le_bytes(q[..4].try_into().unwrap()) as usize];
    assert_eq!(&payload[32..64], grid_api::ID.as_ref());
    assert_eq!(&payload[64..69], &[1, 0, 0, 0, GridInstruction::ConsumeRng as u8]);
    assert_eq!(&payload[69..73], &1u32.to_le_bytes());
    assert_eq!(&payload[73..105], round_pda(id).0.as_ref());
    assert_eq!(&payload[105..107], &[0, 1]);
    assert_eq!(&payload[107..], [&8u32.to_le_bytes()[..], &id.to_le_bytes()].concat().as_slice());

    // Re-request only after `rerequest_after_slots`.
    let retry = end + default_params().rerequest_after_slots;
    game.warp_to(retry - 1);
    assert_eq!(error_code(game.request_rng(ADMIN)), GridError::RngRequested as u32);
    game.warp_to(retry);
    game.request_rng(ADMIN).unwrap();

    // Never once the value is set.
    game.vrf_answer(id, [3; 32]).unwrap();
    game.warp_to(retry + default_params().rerequest_after_slots);
    assert_eq!(error_code(game.request_rng(ADMIN)), GridError::RngRequested as u32);
}

#[test]
fn t6_request_rng_rejects_other_queue() {
    let (mut game, id) = setup();
    game.warp_to(game.board().end_slot);
    let mut ix = request_rng(ADMIN, id);
    ix.accounts[5].pubkey = Pubkey::new_unique();
    assert!(game.send(&[ix], ADMIN).is_err());
}
