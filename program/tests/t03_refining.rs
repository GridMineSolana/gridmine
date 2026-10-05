//! T3: refining. Claiming unrefined $GRID pays a 10% fee to the other unrefined holders, never
//! back to the claimer (F15). Nothing is created or lost.
mod common;

use common::*;
use grid_api::prelude::*;

#[test]
fn t3_refining_fee_goes_to_others() {
    let mut game = Game::ready();
    game.fund_reserve(200_000_000 * 1_000_000);
    let (a, b) = (game.user(), game.user());

    // Both miners on the same split tile, so both earn unrefined $GRID.
    let round_id = game.board().round_id;
    let w = (0..TILES as u64).find(|&t| game.round(round_id).is_split_reward(t as usize)).unwrap();
    game.play_round(&[(a, 10_000_000, tile(w)), (b, 30_000_000, tile(w))], w);
    game.checkpoint(a, round_id).unwrap();
    game.checkpoint(b, round_id).unwrap();
    let (ua, ub) = (game.miner(a).unrefined, game.miner(b).unrefined);
    assert!(ua > 0 && ub > 0);
    let mint = game.token_mint;

    // A claims half, then the rest.
    game.send(&[claim_token(a, mint, spl_token_2022::ID, 5_000)], a).unwrap();
    assert_eq!(game.grid_balance(a), ua / 2 - ua / 2 / 10);
    game.send(&[claim_token(a, mint, spl_token_2022::ID, 10_000)], a).unwrap();
    let a_total = game.grid_balance(a);
    assert!(a_total <= ua - ua / 10 + 1, "claimer got its own fee back: {a_total}");
    assert_eq!(game.miner(a).unrefined + game.miner(a).refined, 0);

    // B is the last holder: no fee, and it receives A's fees.
    game.send(&[claim_token(b, mint, spl_token_2022::ID, 10_000)], b).unwrap();
    let b_total = game.grid_balance(b);
    let fees = ua - a_total;
    assert!(b_total <= ub + fees && b_total + 2 >= ub + fees, "b {b_total} vs {}", ub + fees);

    let t = game.treasury();
    assert!(t.total_unrefined == 0 && t.total_refined <= 2);
}

