//! T2: reserve emission. e = min(reserve * k * boost, cap); 1/6 to the Vault; nothing when the
//! winning tile is empty (F1).
mod common;

use common::*;
use grid_api::prelude::*;

const RESERVE: u64 = 200_000_000 * 1_000_000; // 200M $GRID

/// Plays one round with a single miner on tile 3 and returns (emission, round reward).
fn emit(game: &mut Game, winning_tile: u64) -> (u64, u64) {
    let miner = game.user();
    let before = game.treasury();
    let round_id = game.play_round(&[(miner, 10_000_000, tile(3))], winning_tile);
    let after = game.treasury();
    let round = game.round(round_id);
    let e = before.reserve - after.reserve;
    let vault_part = after.vault[VAULT_GRID] - before.vault[VAULT_GRID];
    assert_eq!(vault_part, e * 1667 / 10_000);
    assert_eq!(round.token_reward, e - vault_part);

    // The only miner on the winning tile gets the whole reward (split or solo).
    game.checkpoint(miner, round_id).unwrap();
    assert_eq!(game.miner(miner).unrefined, round.token_reward);
    assert_eq!(game.treasury().total_unrefined, after.total_unrefined + round.token_reward);
    (e, round.token_reward)
}

#[test]
fn t2_boosted_emission() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    assert_eq!(game.treasury().reserve, RESERVE);
    let (e, _) = emit(&mut game, 3);
    assert_eq!(e, 6_000 * 1_000_000); // 3,000 base x2 boost
}

#[test]
fn t2_unboosted_emission() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    game.set_params(Params { boost_until: 0, ..default_params() }).unwrap();
    let (e, _) = emit(&mut game, 3);
    assert_eq!(e, 3_000 * 1_000_000);
}

#[test]
fn t2_emission_capped() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    game.set_params(Params { max_emission: 1_000 * 1_000_000, ..default_params() }).unwrap();
    let (e, _) = emit(&mut game, 3);
    assert_eq!(e, 1_000 * 1_000_000);
}

#[test]
fn t2_empty_winning_tile_emits_nothing() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    let (e, reward) = emit(&mut game, 4);
    assert_eq!((e, reward), (0, 0));
    assert_eq!(game.treasury().reserve, RESERVE);
}

#[test]
fn t2_unclaimed_reward_returns_to_reserve_on_close() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    let miner = game.user();
    let round_id = game.play_round(&[(miner, 10_000_000, tile(3))], 3);
    let round = game.round(round_id);
    let reserve = game.treasury().reserve;
    // Nobody checkpoints; after expiry the reward goes back to the reserve.
    game.warp_to(round.expires_at + 1);
    game.send(&[close(ADMIN, round_id, round.rent_payer)], ADMIN).unwrap();
    assert_eq!(game.treasury().reserve, reserve + round.token_reward);
}
