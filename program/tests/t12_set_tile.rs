//! T12: SetTile accepts plain SPL and Token-2022 mints, rejects mints that would break transfers,
//! and refuses to swap a tile that still holds tokens.
mod common;

use common::*;
use grid_api::prelude::*;
use solana_sdk::pubkey::Pubkey;

fn set(game: &mut Game, index: u64, mint: Pubkey) -> TxResult {
    game.send(&[set_tile(ADMIN, index, mint)], ADMIN)
}

#[test]
fn t12_accepts_spl_and_plain_t22() {
    let mut game = Game::new();
    let spl = game.create_mint(false, 8, Ext::None);
    let t22 = game.create_mint(true, 9, Ext::None);
    let hook_unset = game.create_mint(true, 6, Ext::Hook(None));
    // Allowed on purpose: xStocks have a permanent delegate (custody risk noted in token.rs).
    let delegate = game.create_mint(true, 8, Ext::PermanentDelegate);
    for (i, mint) in [spl, t22, hook_unset, delegate].into_iter().enumerate() {
        set(&mut game, i as u64, mint).unwrap();
    }
    let tiles = game.config().tiles;
    assert_eq!((tiles[0].mint, tiles[0].is_t22, tiles[0].decimals), (spl, 0, 8));
    assert_eq!((tiles[1].mint, tiles[1].is_t22, tiles[1].decimals), (t22, 1, 9));
    assert_eq!((tiles[2].mint, tiles[2].is_t22, tiles[2].decimals), (hook_unset, 1, 6));
}

#[test]
fn t12_rejects_bad_extensions() {
    let mut game = Game::new();
    for ext in [Ext::Hook(Some(Pubkey::new_unique())), Ext::TransferFee, Ext::NonTransferable, Ext::DefaultFrozen] {
        let mint = game.create_mint(true, 6, ext);
        assert_eq!(error_code(set(&mut game, 0, mint)), GridError::UnsupportedMint as u32);
    }
}

#[test]
fn t12_rejects_shared_mints() {
    let mut game = Game::new();
    let mint = game.create_mint(false, 6, Ext::None);
    set(&mut game, 0, mint).unwrap();
    set(&mut game, 0, mint).unwrap(); // same tile again is fine
    assert_eq!(error_code(set(&mut game, 1, mint)), GridError::UnsupportedMint as u32);
    let grid = game.token_mint;
    assert_eq!(error_code(set(&mut game, 1, grid)), GridError::UnsupportedMint as u32);
    assert_eq!(error_code(set(&mut game, 1, SOL_MINT)), GridError::UnsupportedMint as u32);
}

#[test]
fn t12_rejects_non_token_account() {
    let mut game = Game::new();
    let not_a_mint = game.user();
    assert_eq!(error_code(set(&mut game, 0, not_a_mint)), GridError::UnsupportedMint as u32);
}

#[test]
fn t12_tile_must_be_empty() {
    let mut game = Game::ready();
    let mint = game.create_mint(false, 6, Ext::None);
    game.update::<Treasury>(treasury_pda().0, |t| t.pot_tokens[3] = 1);
    game.update::<Treasury>(treasury_pda().0, |t| t.vault[4] = 1);
    assert_eq!(error_code(set(&mut game, 3, mint)), GridError::TileNotEmpty as u32);
    assert_eq!(error_code(set(&mut game, 4, mint)), GridError::TileNotEmpty as u32);
    set(&mut game, 5, mint).unwrap();
}

#[test]
fn t12_admin_only_and_index_bounds() {
    let mut game = Game::new();
    let mint = game.create_mint(false, 6, Ext::None);
    let user = game.user();
    let res = game.send(&[set_tile(user, 0, mint)], user);
    assert_eq!(error_code(res), GridError::NotAuthorized as u32);
    assert_eq!(error_code(set(&mut game, TILES as u64, mint)), GridError::InvalidTile as u32);
}

#[test]
fn t12_update_config_validates() {
    let mut game = Game::new();
    let bad_split = Params { team_bps: 1001, ..default_params() };
    assert_eq!(error_code(game.set_params(bad_split)), GridError::InvalidConfig as u32);
    let bad_fee = Params { admin_fee_bps: 501, ..default_params() };
    assert_eq!(error_code(game.set_params(bad_fee)), GridError::InvalidConfig as u32);
    game.set_params(default_params()).unwrap();
}
