//! T13: SetTokenMint. The admin can switch `$GRID` any number of times. Old Treasury tokens go to
//! the admin; the admin pays in new tokens for what miners are still owed, so claims keep working.
mod common;

use common::*;
use grid_api::prelude::*;

#[test]
fn t13_switch_before_funding_repeatable_and_guarded() {
    let mut game = Game::ready();
    let old = game.token_mint;

    // Admin only.
    let new = game.create_mint(false, 9, Ext::None);
    let user = game.user();
    assert!(game.send(&[set_token_mint(user, old, spl_token_2022::ID, new, spl_token::ID)], user).is_err());

    // Nothing held yet: just switches (SPL, 9 decimals).
    game.send(&[set_token_mint(ADMIN, old, spl_token_2022::ID, new, spl_token::ID)], ADMIN).unwrap();
    let c = game.config();
    assert_eq!((c.token_mint, c.token_program, c.token_decimals), (new, spl_token::ID, 9));

    // Again (Token-2022, 6 decimals): repeatable.
    let newer = game.create_mint(true, 6, Ext::None);
    game.send(&[set_token_mint(ADMIN, new, spl_token::ID, newer, spl_token_2022::ID)], ADMIN).unwrap();
    let c = game.config();
    assert_eq!((c.token_mint, c.token_program, c.token_decimals), (newer, spl_token_2022::ID, 6));

    // Rejected: a tile mint, wSOL, the current mint, a transfer-fee mint.
    let tile_mint = game.config().tiles[0].mint;
    let fee_mint = game.create_mint(true, 6, Ext::TransferFee);
    for (mint, program) in [(tile_mint, spl_token::ID), (SOL_MINT, spl_token::ID), (newer, spl_token_2022::ID), (fee_mint, spl_token_2022::ID)] {
        assert!(game.send(&[set_token_mint(ADMIN, newer, spl_token_2022::ID, mint, program)], ADMIN).is_err());
    }
}

#[test]
fn t13_switch_after_funding_keeps_claims_whole() {
    let mut game = Game::ready();
    game.fund_reserve(200_000_000 * 1_000_000);
    let old = game.token_mint;
    let p = spl_token_2022::ID;

    // A miner earns unrefined $GRID and leaves it unclaimed.
    let a = game.user();
    let round_id = game.board().round_id;
    let w = (0..TILES as u64).find(|&t| game.round(round_id).is_split_reward(t as usize)).unwrap();
    game.play_round(&[(a, 10_000_000, tile(w))], w);
    game.checkpoint(a, round_id).unwrap();
    let owed_to_a = game.miner(a).unrefined;
    assert!(owed_to_a > 0);

    let t = game.treasury();
    let held = game.token_balance(game_treasury(), old, p);
    let owed = held - t.reserve - t.vault[VAULT_GRID];
    assert!(owed >= owed_to_a);

    // Admin holds enough new tokens; the switch moves old out and `owed` new in.
    let new = game.create_mint(true, 6, Ext::None);
    game.mint_to(ADMIN, new, p, owed);
    game.send(&[set_token_mint(ADMIN, old, p, new, p)], ADMIN).unwrap();
    assert_eq!(game.token_balance(ADMIN, old, p), held);
    assert_eq!(game.token_balance(game_treasury(), old, p), 0);
    assert_eq!(game.token_balance(game_treasury(), new, p), owed);
    assert_eq!(game.token_balance(ADMIN, new, p), 0);
    let t = game.treasury();
    assert_eq!((t.reserve, t.vault[VAULT_GRID]), (0, 0));

    // The miner claims in the new mint.
    game.token_mint = new;
    game.send(&[claim_token(a, new, p, 10_000)], a).unwrap();
    assert_eq!(game.grid_balance(a), owed_to_a);

    // The reserve refills in the new mint and rounds keep emitting.
    game.fund_reserve(1_000_000 * 1_000_000);
    assert_eq!(game.treasury().reserve, 1_000_000 * 1_000_000);
    let id = game.board().round_id;
    game.play_round(&[(a, 10_000_000, tile(w))], w);
    assert!(game.round(id).token_reward > 0 || game.round(id).void != 0);

    // Not enough new tokens: the switch fails and nothing changes.
    let newer = game.create_mint(true, 6, Ext::None);
    let before = game.config();
    let t = game.treasury();
    if game.token_balance(game_treasury(), new, p) > t.reserve + t.vault[VAULT_GRID] {
        assert!(game.send(&[set_token_mint(ADMIN, new, p, newer, p)], ADMIN).is_err());
        assert_eq!(game.config().token_mint, before.token_mint);
    }
}

fn game_treasury() -> steel::Pubkey {
    treasury_pda().0
}
