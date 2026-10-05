//! T5: the VRF never answers. Reset waits until `end_slot + void_after_slots`, then voids the
//! round: 100% refund, no fees, no emission. A late answer is ignored.
mod common;

use common::*;
use grid_api::prelude::*;

#[test]
fn t5_void_refunds_everything() {
    let mut game = Game::ready();
    game.fund_reserve(200_000_000 * 1_000_000);
    let round_id = game.board().round_id;
    let all = (1 << TILES) - 1;
    let deploys = [(game.user(), 10_000_000, tile(0) | tile(5)), (game.user(), 30_000_000, tile(5)), (game.user(), 7_777_777, all)];
    for &(miner, amount, mask) in &deploys {
        game.deploy(miner, amount, mask).unwrap();
    }

    // Asked, never answered.
    let end = game.board().end_slot;
    game.warp_to(end);
    game.request_rng(ADMIN).unwrap();
    let treasury = game.treasury();
    let treasury_lamports = game.lamports(treasury_pda().0);

    let void_slot = end + default_params().void_after_slots;
    for slot in [end + default_params().intermission_slots, void_slot - 1] {
        game.warp_to(slot);
        assert_eq!(error_code(game.send(&[reset(ADMIN, round_id)], ADMIN)), GridError::RngNotReady as u32);
    }
    // An answer at the deadline, before Reset, is too late: the round still voids.
    game.warp_to(void_slot);
    game.vrf_answer(round_id, [4; 32]).unwrap();
    assert_eq!(game.round(round_id).rng, [0; 32]);
    game.send(&[reset(ADMIN, round_id)], ADMIN).unwrap();

    let round = game.round(round_id);
    assert_eq!((round.void, round.winning_tile), (1, u64::MAX));
    assert_eq!((round.admin_fee, round.protocol_fee, round.token_reward), (0, 0, 0));
    assert_eq!(round.total_returned_sol, round.total_deployed());
    assert_eq!(game.treasury(), treasury);
    assert_eq!(game.lamports(treasury_pda().0), treasury_lamports);
    assert_eq!(game.board().round_id, round_id + 1);

    // A late answer can't un-void the round.
    game.vrf_answer(round_id, [5; 32]).unwrap();
    assert_eq!(game.round(round_id).rng, [0; 32]);

    // Every miner gets back exactly what it deployed; nothing is left in the round.
    for &(miner, amount, mask) in &deploys {
        let before = game.lamports(miner);
        game.send(&[checkpoint(ADMIN, miner, round_id)], ADMIN).unwrap();
        assert_eq!(game.lamports(miner), before + amount * mask.count_ones() as u64);
        assert_eq!(game.miner(miner).unrefined, 0);
    }
    let rent = game.svm.minimum_balance_for_rent_exemption(8 + core::mem::size_of::<Round>());
    assert_eq!(game.lamports(round_pda(round_id).0), rent);
}

/// A crank outage past the old 1-day expiry still refunds: the claim window starts at Reset.
#[test]
fn t5_late_reset_still_refunds() {
    let mut game = Game::ready();
    let miner = game.user();
    game.deploy(miner, 10_000_000, tile(0)).unwrap();
    let round_id = game.board().round_id;
    let late = game.board().end_slot + ONE_DAY_SLOTS + 1;
    game.warp_to(late);
    game.send(&[reset(ADMIN, round_id)], ADMIN).unwrap();
    let round = game.round(round_id);
    assert_eq!((round.void, round.expires_at), (1, late + ONE_DAY_SLOTS));
    let res = game.send(&[close(ADMIN, round_id, round.rent_payer)], ADMIN);
    assert_eq!(error_code(res), GridError::RoundNotOver as u32);
    let before = game.lamports(miner);
    game.send(&[checkpoint(ADMIN, miner, round_id)], ADMIN).unwrap();
    assert_eq!(game.lamports(miner), before + 10_000_000);
}

/// A miner opened by Automate is refunded for its round-0 deposit (round/checkpoint ids start unset).
#[test]
fn t5_automation_round_zero_refund() {
    let mut game = Game::ready();
    let user = game.user();
    let args = Automate {
        amount: 10_000_000,
        deposit: SOL,
        fee: 0,
        mask: tile(3),
        strategy: AutomationStrategy::Preferred as u64,
        reload: 0,
        solo_tiles: 0,
        split_tiles: 0,
    };
    game.send(&[automate(user, WORKER, args)], user).unwrap();
    assert_eq!(game.board().round_id, 0);
    game.send(&[deploy(WORKER, user, 0, 0, 0)], WORKER).unwrap();
    assert_eq!(game.round(0).deployed[3], 10_000_000);

    game.warp_to(game.round(0).rng_deadline);
    game.send(&[reset(ADMIN, 0)], ADMIN).unwrap();
    let before = game.lamports(user);
    game.send(&[checkpoint(ADMIN, user, 0)], ADMIN).unwrap();
    assert_eq!(game.lamports(user), before + 10_000_000);
}
