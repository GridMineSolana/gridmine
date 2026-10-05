//! T11: SetPause blocks Deploy only. The crank (RequestRng, ConsumeRng, Reset), Checkpoint and
//! claims keep working, so user funds are never stuck.
mod common;

use common::*;
use grid_api::prelude::*;

#[test]
fn t11_pause_blocks_deploy_only() {
    let mut game = Game::ready();
    game.fund_reserve(200_000_000 * 1_000_000);
    let (miner, late) = (game.user(), game.user());
    let round_id = game.board().round_id;
    game.deploy(miner, 10_000_000, tile(2)).unwrap();

    assert_eq!(error_code(game.send(&[set_pause(miner, true)], miner)), GridError::NotAuthorized as u32);
    game.send(&[set_pause(ADMIN, true)], ADMIN).unwrap();
    assert_eq!(error_code(game.deploy(late, 10_000_000, tile(2))), GridError::Paused as u32);

    game.vrf_tile(round_id, 2);
    game.reset().unwrap();
    game.checkpoint(miner, round_id).unwrap();
    assert!(game.miner(miner).unrefined > 0);
    game.send(&[claim_sol(miner)], miner).unwrap();
    game.send(&[claim_token(miner, game.token_mint, spl_token_2022::ID, 10_000)], miner).unwrap();
    assert!(game.grid_balance(miner) > 0);
    assert_eq!(error_code(game.deploy(late, 10_000_000, tile(2))), GridError::Paused as u32);

    game.send(&[set_pause(ADMIN, false)], ADMIN).unwrap();
    game.deploy(late, 10_000_000, tile(2)).unwrap();
}
