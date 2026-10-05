//! T4: one full round on the real VRF path (mock VRF at the MagicBlock id). After every step the
//! Treasury's SOL and `$GRID` match its books and the round holds rent + what it still owes.
mod common;

use common::*;
use grid_api::prelude::*;

const RESERVE: u64 = 200_000_000 * 1_000_000;

fn check(game: &Game, round_id: u64, owed_sol: u64) {
    let (sol, grid) = treasury_books(game, &[round_id]);
    assert_eq!(game.lamports(treasury_pda().0), sol);
    assert_eq!(game.grid_balance(treasury_pda().0), grid);
    let rent = game.svm.minimum_balance_for_rent_exemption(8 + core::mem::size_of::<Round>());
    assert_eq!(game.lamports(round_pda(round_id).0), rent + owed_sol);
}

#[test]
fn t4_full_round_lifecycle() {
    // Initialize, SetTile x25, unpause, FundReserve.
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    let round_id = game.board().round_id;
    check(&game, round_id, 0);

    // Three miners deploy. a and b share a split tile w; c only loses.
    let w = (0..TILES as u64).find(|&t| game.round(round_id).is_split_reward(t as usize)).unwrap();
    let x = (w + 1) % TILES as u64;
    let deploys = [(game.user(), 10_000_000, tile(w)), (game.user(), 30_000_000, tile(w) | tile(x)), (game.user(), 20_000_000, tile(x))];
    for &(miner, amount, mask) in &deploys {
        game.deploy(miner, amount, mask).unwrap();
    }
    let deposited = 10_000_000 + 60_000_000 + 20_000_000;
    check(&game, round_id, deposited);

    // RequestRng only after deposits close; the payer funds the VRF fee.
    assert_eq!(error_code(game.request_rng(ADMIN)), GridError::RoundNotOver as u32);
    let payer = game.user();
    let queue_before = game.lamports(VRF_QUEUE);
    game.warp_to(game.board().end_slot);
    game.request_rng(payer).unwrap();
    assert_eq!(game.lamports(VRF_QUEUE), queue_before + 500_000);
    check(&game, round_id, deposited);

    // The VRF answers; Reset settles the round on tile w.
    let rng = rng_for_tile(round_id, w);
    game.vrf_answer(round_id, rng).unwrap();
    assert_eq!(game.round(round_id).rng, rng);
    game.reset().unwrap();
    let round = game.round(round_id);
    assert_eq!(round.winning_tile, w);
    assert!(round.token_reward > 0);
    check(&game, round_id, round.total_returned_sol);

    // Checkpoint (by a bot, so the miners pay no fees): each gets its exact share.
    let mut owed = round.total_returned_sol;
    for &(miner, amount, mask) in &deploys {
        let expected: u64 = (0..TILES)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| {
                let (_, _, back) = tile_split(round.deployed[i], i as u64 == w, 100, 1000).unwrap();
                mul_div(amount, back, round.deployed[i]).unwrap()
            })
            .sum();
        let before = game.lamports(miner);
        game.send(&[checkpoint(ADMIN, miner, round_id)], ADMIN).unwrap();
        assert_eq!(game.lamports(miner), before + expected);
        owed -= expected;
        check(&game, round_id, owed);
    }
    assert!(owed < 4, "dust {owed}");

    // ClaimSol (nothing held back here) and ClaimToken (a pays the refining fee to b).
    let mint = game.token_mint;
    for &(miner, _, _) in &deploys {
        assert_eq!(game.miner(miner).rewards_sol, 0);
        game.send(&[claim_sol(miner)], miner).unwrap();
        if game.miner(miner).unrefined > 0 {
            game.send(&[claim_token(miner, mint, spl_token_2022::ID, 10_000)], miner).unwrap();
        }
        check(&game, round_id, owed);
    }
    let paid: u64 = deploys.iter().map(|d| game.grid_balance(d.0)).sum();
    assert!(paid > 0);
    assert_eq!(paid + game.grid_balance(treasury_pda().0), RESERVE);

    // Close after expiry: dust to buyback, rent back, uncredited `$GRID` (none) to the reserve.
    game.warp_to(round.expires_at + 1);
    let buyback = game.treasury().buyback_sol;
    game.send(&[close(ADMIN, round_id, round.rent_payer)], ADMIN).unwrap();
    assert!(game.svm.get_account(&round_pda(round_id).0).is_none_or(|a| a.lamports == 0));
    assert_eq!(game.treasury().buyback_sol, buyback + owed);
    let (sol, grid) = treasury_books(&game, &[]);
    assert_eq!((game.lamports(treasury_pda().0), game.grid_balance(treasury_pda().0)), (sol, grid));
}

/// Anyone can create the wSOL ATAs before Initialize; Initialize must still succeed.
#[test]
fn t4_initialize_with_precreated_wsol_atas() {
    let mut game = Game::uninitialized();
    let attacker = game.user();
    let ixs: Vec<_> = [treasury_pda().0, swap_pda().0]
        .iter()
        .map(|o| spl_associated_token_account::instruction::create_associated_token_account(&attacker, o, &SOL_MINT, &spl_token::ID))
        .collect();
    game.send(&ixs, attacker).unwrap();
    game.init().unwrap();
}

/// Without an automation only the authority deploys for its miner; an empty deploy starts no round.
#[test]
fn t4_deploy_guards() {
    let mut game = Game::ready();
    let (victim, attacker) = (game.user(), game.user());
    let round_id = game.board().round_id;
    game.deploy(victim, 5_000_000, 0).unwrap();
    assert_eq!(game.board().end_slot, u64::MAX);
    let res = game.send(&[deploy(attacker, victim, 5_000_000, round_id, tile(0))], attacker);
    assert_eq!(error_code(res), GridError::NotAuthorized as u32);
    game.deploy(victim, 10 * SOL, tile(0)).unwrap();
    assert_eq!(game.round(round_id).deployed[0], 10 * SOL);
}
