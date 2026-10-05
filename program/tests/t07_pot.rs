//! T7: BuyAsset(pot) via the mock, the tile wins, Checkpoint picks the SOL-weighted winner,
//! ClaimPot pays the winner's Token-2022 ATA once, and an unclaimed pot returns on Close.
mod common;

use common::*;
use grid_api::prelude::*;

const T22: solana_sdk::pubkey::Pubkey = spl_token_2022::ID;

#[test]
fn t7_pot_buy_win_claim_close() {
    let mut game = Game::ready();
    let mint = game.create_mint(true, 8, Ext::PermanentDelegate); // xStocks-like
    game.send(&[set_tile(ADMIN, 3, mint)], ADMIN).unwrap();
    let (a, b, c) = (game.user(), game.user(), game.user());

    // Losing SOL feeds every tile's pot; BuyAsset(pot) swaps tile 3's.
    let r = game.play_round(&[(c, SOL, tile(0) | tile(1))], 0);
    game.checkpoint(c, r).unwrap();
    let pot_sol = game.treasury().pot_sol[3];
    assert!(pot_sol > 0);
    let lamports = game.lamports(treasury_pda().0);
    game.buy(BUY_POT, 3, pot_sol, 1_000_000, 0).unwrap();
    let t = game.treasury();
    assert_eq!((t.pot_sol[3], t.pot_tokens[3]), (0, 1_000_000));
    assert_eq!(game.lamports(treasury_pda().0), lamports - pot_sol);
    game.assert_tile_books(3);

    // Tile 3 wins: the round takes the pot; the tile can't be swapped while the round owes it.
    let r = game.play_round(&[(a, 10_000_000, tile(3)), (b, 20_000_000, tile(3)), (c, 10_000_000, tile(5))], 3);
    let round = game.round(r);
    assert_eq!(round.pot_amount, 1_000_000);
    assert_eq!((game.treasury().pot_tokens[3], game.treasury().owed[3]), (0, 1_000_000));
    game.assert_tile_books(3);
    let other = game.create_mint(false, 6, Ext::None);
    assert_eq!(error_code(game.send(&[set_tile(ADMIN, 3, other)], ADMIN)), GridError::TileNotEmpty as u32);
    let res = game.send(&[claim_pot(WORKER, r, a, mint, T22)], WORKER);
    assert_eq!(error_code(res), GridError::NothingToClaim as u32);

    // Checkpoint sets the winner: a owns samples [0, 10M), b [10M, 30M).
    let sample = draw(&round.rng, r, b"pot", round.deployed[3]);
    let (winner, loser) = if sample < 10_000_000 { (a, b) } else { (b, a) };
    for m in [a, b, c] {
        game.checkpoint(m, r).unwrap();
    }
    assert_eq!(game.round(r).pot_winner, winner);

    // Paying the loser fails; anyone can pay the winner (signer funds the ATA); only once.
    assert!(game.send(&[claim_pot(WORKER, r, loser, mint, T22)], WORKER).is_err());
    game.send(&[claim_pot(WORKER, r, winner, mint, T22)], WORKER).unwrap();
    assert_eq!(game.token_balance(winner, mint, T22), 1_000_000);
    assert_eq!(game.token_balance(loser, mint, T22), 0);
    let res = game.send(&[claim_pot(WORKER, r, winner, mint, T22)], WORKER);
    assert_eq!(error_code(res), GridError::AlreadyClaimed as u32);
    assert_eq!(game.treasury().owed[3], 0);
    game.assert_tile_books(3);

    // Next win nobody claims: Close returns the pot to tile 3's books.
    let pot_sol = game.treasury().pot_sol[3];
    game.buy(BUY_POT, 3, pot_sol, 500_000, 0).unwrap();
    let r = game.play_round(&[(a, 10_000_000, tile(3))], 3);
    game.checkpoint(a, r).unwrap();
    assert_eq!(game.round(r).pot_winner, a);
    let round = game.round(r);
    game.warp_to(round.expires_at + 1);
    game.send(&[close(ADMIN, r, round.rent_payer)], ADMIN).unwrap();
    let t = game.treasury();
    assert_eq!((t.pot_tokens[3], t.owed[3]), (500_000, 0));
    game.assert_tile_books(3);
}
