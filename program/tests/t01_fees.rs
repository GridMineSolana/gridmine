//! T1: SOL conservation through Reset + Checkpoint. Every lamport deployed ends up in a Treasury
//! bucket (admin fee included), back with a miner, or as dust left in the round.
mod common;

use common::*;
use grid_api::prelude::*;

fn run(deploys: &[(u64, u64)], winning_tile: u64) {
    let mut game = Game::ready();
    let miners: Vec<_> = deploys.iter().map(|_| game.user()).collect();
    let round_id = game.board().round_id;
    let round_key = round_pda(round_id).0;
    let round_rent = game.lamports(round_key);

    for (miner, &(amount, mask)) in miners.iter().zip(deploys) {
        game.deploy(*miner, amount, mask).unwrap();
    }
    let deposited = game.lamports(round_key) - round_rent;
    let expected: u64 = deploys.iter().map(|(a, m)| a * m.count_ones() as u64).sum();
    assert_eq!(deposited, expected);

    let treasury_before = game.lamports(treasury_pda().0);
    game.vrf_tile(round_id, winning_tile);
    game.reset().unwrap();

    let round = game.round(round_id);
    let t = game.treasury();
    let fees = game.lamports(treasury_pda().0) - treasury_before;
    assert_eq!(t.admin_sol, round.admin_fee);
    assert_eq!(fees, round.admin_fee + round.protocol_fee);
    assert_eq!(t.buyback_sol + t.vault_sol + t.team_sol + t.pot_sol.iter().sum::<u64>(), round.protocol_fee);
    assert_eq!(fees + round.total_returned_sol, deposited);

    // Checkpoint everyone: returns come out of the round account.
    let mut returned = 0;
    for miner in &miners {
        let before = game.lamports(round_key);
        game.checkpoint(*miner, round_id).unwrap();
        returned += before - game.lamports(round_key);
    }
    let dust = game.lamports(round_key) - round_rent;
    assert_eq!(returned + dust, round.total_returned_sol);
    let pairs: u64 = deploys.iter().map(|(_, m)| m.count_ones() as u64).sum();
    assert!(dust < pairs.max(1), "dust {dust} >= miner-tile pairs {pairs}");
}

#[test]
fn t1_one_tile_wins_pays_admin_fee_only() {
    run(&[(5_000_000, tile(3)), (7_777_777, tile(3))], 3);
}

#[test]
fn t1_one_tile_loses() {
    run(&[(5_000_001, tile(3))], 4);
}

#[test]
fn t1_all_25_tiles_dust_amounts() {
    let all = (1 << TILES) - 1;
    run(&[(5_000_003, all), (5_123_457, all), (9_999_999, tile(0) | tile(24))], 24);
}

#[test]
fn t1_no_deposits_cannot_reset() {
    let mut game = Game::ready();
    let res = game.send(&[reset(ADMIN, 0)], ADMIN);
    assert_eq!(error_code(res), GridError::RoundNotOver as u32);
}

#[test]
fn t1_min_deploy_enforced() {
    let mut game = Game::ready();
    let miner = game.user();
    assert_eq!(error_code(game.deploy(miner, 4_999_999, tile(0))), GridError::AmountTooSmall as u32);
}

#[test]
fn t1_withdraw_team_pays_each_bucket_to_its_collector() {
    let mut game = Game::ready();
    let miner = game.user();
    game.play_round(&[(miner, 100_000_000, tile(1) | tile(2))], 1);
    let t = game.treasury();
    assert!(t.admin_sol > 0 && t.team_sol > 0);
    let (collector, team) = (game.lamports(COLLECTOR), game.lamports(TEAM));

    // Admin only, right collector only, never more than the bucket.
    let user = game.user();
    assert_eq!(error_code(game.send(&[withdraw_team(user, COLLECTOR, 1, 0)], user)), GridError::NotAuthorized as u32);
    assert!(game.send(&[withdraw_team(ADMIN, TEAM, 1, 0)], ADMIN).is_err());
    let res = game.send(&[withdraw_team(ADMIN, COLLECTOR, t.admin_sol + 1, 0)], ADMIN);
    assert_eq!(error_code(res), GridError::InsufficientFunds as u32);

    game.send(&[withdraw_team(ADMIN, COLLECTOR, t.admin_sol, 0)], ADMIN).unwrap();
    game.send(&[withdraw_team(ADMIN, TEAM, t.team_sol, 1)], ADMIN).unwrap();
    let after = game.treasury();
    assert_eq!((after.admin_sol, after.team_sol), (0, 0));
    assert_eq!(game.lamports(COLLECTOR), collector + t.admin_sol);
    assert_eq!(game.lamports(TEAM), team + t.team_sol);
    assert_eq!(game.lamports(treasury_pda().0), treasury_books(&game, &[]).0);
}
