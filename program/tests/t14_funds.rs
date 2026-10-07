//! T14: Fund (anyone) and Withdraw (admin) for every Treasury bucket.
mod common;

use common::*;
use grid_api::prelude::*;

#[test]
fn t14_sol_buckets() {
    let mut game = Game::ready();
    let user = game.user();
    let treasury = treasury_pda().0;

    // Anyone funds the vault; the worker can then buy with it.
    let before = game.lamports(treasury);
    game.send(&[fund(user, BUCKET_VAULT_SOL, 0, SOL / 2, None)], user).unwrap();
    assert_eq!(game.treasury().vault_sol, SOL / 2);
    assert_eq!(game.lamports(treasury), before + SOL / 2);
    game.buy(BUY_VAULT, 3, SOL / 2, 1_000, 0).unwrap();
    assert_eq!(game.treasury().vault[3], 1_000);

    // Pot SOL for a tile: funded, then withdrawn by the admin to any wallet.
    game.send(&[fund(user, BUCKET_POT_SOL, 7, SOL, None)], user).unwrap();
    assert_eq!(game.treasury().pot_sol[7], SOL);
    let to = game.user();
    assert!(game.send(&[withdraw(user, BUCKET_POT_SOL, 7, SOL, user, None)], user).is_err());
    assert!(game.send(&[withdraw(ADMIN, BUCKET_POT_SOL, 7, SOL + 1, to, None)], ADMIN).is_err());
    let before = game.lamports(to);
    game.send(&[withdraw(ADMIN, BUCKET_POT_SOL, 7, SOL, to, None)], ADMIN).unwrap();
    assert_eq!((game.treasury().pot_sol[7], game.lamports(to)), (0, before + SOL));

    // Bad bucket, bad tile, zero amount.
    assert!(game.send(&[fund(user, 8, 0, SOL, None)], user).is_err());
    assert!(game.send(&[fund(user, BUCKET_POT_SOL, TILES as u64, SOL, None)], user).is_err());
    assert!(game.send(&[fund(user, BUCKET_VAULT_SOL, 0, 0, None)], user).is_err());
}

#[test]
fn t14_token_buckets() {
    let mut game = Game::ready();
    let user = game.user();
    let to = game.user();
    let tile = game.config().tiles[3];
    let (mint, p) = (tile.mint, tile.token_program());
    let grid = (game.token_mint, spl_token_2022::ID);

    // Vault asset and pot tokens for tile 3.
    game.mint_to(user, mint, p, 3_000);
    game.send(&[fund(user, BUCKET_VAULT_TOKENS, 3, 2_000, Some((mint, p)))], user).unwrap();
    game.send(&[fund(user, BUCKET_POT_TOKENS, 3, 1_000, Some((mint, p)))], user).unwrap();
    let t = game.treasury();
    assert_eq!((t.vault[3], t.pot_tokens[3]), (2_000, 1_000));
    game.assert_tile_books(3);

    // Wrong mint for the bucket.
    game.mint_to(user, grid.0, grid.1, 0);
    assert!(game.send(&[fund(user, BUCKET_VAULT_TOKENS, 3, 1, Some(grid))], user).is_err());

    game.mint_to(to, mint, p, 0);
    let dest = ata(&to, &mint, &p);
    assert!(game.send(&[withdraw(user, BUCKET_VAULT_TOKENS, 3, 1, dest, Some((mint, p)))], user).is_err());
    assert!(game.send(&[withdraw(ADMIN, BUCKET_VAULT_TOKENS, 3, 2_001, dest, Some((mint, p)))], ADMIN).is_err());
    game.send(&[withdraw(ADMIN, BUCKET_VAULT_TOKENS, 3, 2_000, dest, Some((mint, p)))], ADMIN).unwrap();
    game.send(&[withdraw(ADMIN, BUCKET_POT_TOKENS, 3, 1_000, dest, Some((mint, p)))], ADMIN).unwrap();
    assert_eq!(game.token_balance(to, mint, p), 3_000);
    let t = game.treasury();
    assert_eq!((t.vault[3], t.pot_tokens[3]), (0, 0));
    game.assert_tile_books(3);

    // $GRID: vault bonus (index 25) and the emission reserve.
    game.mint_grid(user, 500);
    game.send(&[fund(user, BUCKET_VAULT_TOKENS, VAULT_GRID as u64, 200, Some(grid))], user).unwrap();
    game.send(&[fund(user, BUCKET_RESERVE, 0, 300, Some(grid))], user).unwrap();
    let t = game.treasury();
    assert_eq!((t.vault[VAULT_GRID], t.reserve), (200, 300));
    game.mint_to(to, grid.0, grid.1, 0);
    let dest = ata(&to, &grid.0, &grid.1);
    game.send(&[withdraw(ADMIN, BUCKET_RESERVE, 0, 300, dest, Some(grid))], ADMIN).unwrap();
    game.send(&[withdraw(ADMIN, BUCKET_VAULT_TOKENS, VAULT_GRID as u64, 200, dest, Some(grid))], ADMIN).unwrap();
    assert_eq!(game.grid_balance(to), 500);
    let t = game.treasury();
    assert_eq!((t.vault[VAULT_GRID], t.reserve), (0, 0));
}
