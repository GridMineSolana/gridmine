//! T8: the swap helper rejects short output, routes that touch other Treasury accounts, non-worker
//! signers, swaps over the cap or too soon, other programs, and overdrawn buckets.
mod common;

use common::*;
use grid_api::prelude::*;
use solana_sdk::pubkey::Pubkey;

fn send_buy(game: &mut Game, signer: Pubkey, amount_in: u64, route: steel::Instruction) -> TxResult {
    let t = game.config().tiles[2];
    game.send(&[buy_asset(signer, BUY_POT, 2, amount_in, 1000, t.mint, t.token_program(), route)], signer)
}

fn route(game: &mut Game, mode: u8, amount_in: u64, extra: &[Pubkey]) -> steel::Instruction {
    let t = game.config().tiles[2];
    game.route(mode, amount_in, 1000, t.mint, t.token_program(), extra)
}

#[test]
fn t8_swap_guards() {
    let mut game = Game::ready();
    let filler = game.user();
    game.play_round(&[(filler, SOL, tile(0) | tile(1))], 0);
    let amount = game.treasury().pot_sol[2];
    let treasury = treasury_pda().0;
    let before = game.treasury();

    // short: less than min_out reverts.
    assert_eq!(error_code(game.buy(BUY_POT, 2, amount, 1000, 1)), GridError::MinOut as u32);

    // steals / touches: another Treasury token account, the Treasury, or any GRID account.
    let t4 = game.config().tiles[4];
    game.mint_to(treasury, t4.mint, t4.token_program(), 0);
    let victim = ata(&treasury, &t4.mint, &t4.token_program());
    let r = route(&mut game, 2, amount, &[victim]);
    game.after_swap_interval();
    assert_eq!(error_code(send_buy(&mut game, WORKER, amount, r)), GridError::BadRoute as u32);
    for bad in [ata(&treasury, &SOL_MINT, &spl_token::ID), treasury, config_pda().0] {
        let r = route(&mut game, 0, amount, &[bad]);
        assert_eq!(error_code(send_buy(&mut game, WORKER, amount, r)), GridError::BadRoute as u32);
    }

    // Non-worker signer.
    let user = game.user();
    let r = route(&mut game, 0, amount, &[]);
    assert_eq!(error_code(send_buy(&mut game, user, amount, r)), GridError::NotAuthorized as u32);

    // Over the cap; zero; more than the bucket.
    let cap = default_params().max_swap_lamports;
    for (amount_in, err) in [(cap + 1, GridError::SwapLimit), (0, GridError::SwapLimit), (amount + 1, GridError::InsufficientFunds)] {
        let r = route(&mut game, 0, amount_in, &[]);
        assert_eq!(error_code(send_buy(&mut game, WORKER, amount_in, r)), err as u32);
    }

    // Only the Jupiter program id.
    let mut r = route(&mut game, 0, amount, &[]);
    r.program_id = VRF_PROGRAM_ID;
    assert!(send_buy(&mut game, WORKER, amount, r).is_err());
    assert_eq!(game.treasury(), before);

    // Honest passes; another swap before min_swap_interval_slots fails.
    let lamports = game.lamports(treasury);
    game.buy(BUY_POT, 2, amount / 2, 1000, 0).unwrap();
    assert_eq!(game.lamports(treasury), lamports - amount / 2);
    let r = route(&mut game, 0, amount / 2, &[]);
    assert_eq!(error_code(send_buy(&mut game, WORKER, amount / 2, r)), GridError::SwapLimit as u32);
    game.buy(BUY_POT, 2, amount / 2, 1000, 0).unwrap();
    let t = game.treasury();
    assert_eq!((t.pot_sol[2], t.pot_tokens[2]), (amount - amount / 2 * 2, 2000));
    game.assert_tile_books(2);
    assert_eq!(game.token_balance(swap_pda().0, SOL_MINT, spl_token::ID), 0);
}
