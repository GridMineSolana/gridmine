//! T10: Buyback swaps the buyback bucket plus creator fees (surplus lamports + Treasury wSOL) into
//! `$GRID`. The protocol part is split burn / recycle exactly; supply drops by the burn; the
//! creator part goes 100% to the reserve.
mod common;

use common::*;
use grid_api::prelude::*;
use solana_sdk::{pubkey::Pubkey, system_instruction};

const RESERVE: u64 = 200_000_000 * 1_000_000;
const T22: Pubkey = spl_token_2022::ID;

fn supply(game: &Game) -> u64 {
    let data = game.svm.get_account(&game.token_mint).unwrap().data;
    u64::from_le_bytes(data[36..44].try_into().unwrap())
}

fn buyback_ix(game: &mut Game, amount_in: u64, out: u64) -> steel::Instruction {
    let mint = game.token_mint;
    let route = game.route(0, amount_in, out, mint, T22, &[]);
    game.after_swap_interval();
    buyback(WORKER, mint, T22, amount_in, out, route)
}

#[test]
fn t10_buyback_split_and_creator_fees() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    let m = game.user();
    let r = game.play_round(&[(m, 10 * SOL, tile(0) | tile(1))], 0);
    let protocol = game.treasury().buyback_sol;
    assert!(protocol > 0);

    // Creator fees: lamports straight into the Treasury (curve) and wSOL into its ATA (PumpSwap).
    let treasury = treasury_pda().0;
    let treasury_wsol = ata(&treasury, &SOL_MINT, &spl_token::ID);
    let (surplus, wsol) = (300_000_000, 200_000_000);
    game.svm.airdrop(&treasury, surplus).unwrap();
    let wrap = [
        system_instruction::transfer(&ADMIN, &treasury_wsol, wsol),
        spl_token::instruction::sync_native(&spl_token::ID, &treasury_wsol).unwrap(),
    ];
    game.send(&wrap, ADMIN).unwrap();

    // Asking for more than bucket + creator fees fails.
    let amount_in = protocol + surplus + wsol;
    let ix = buyback_ix(&mut game, amount_in + 1, 1);
    assert_eq!(error_code(game.send(&[ix], WORKER)), GridError::InsufficientFunds as u32);

    let out = 1_000 * 1_000_000;
    let (supply_before, reserve_before) = (supply(&game), game.treasury().reserve);
    let ix = buyback_ix(&mut game, amount_in, out);
    game.send(&[ix], WORKER).unwrap();

    let protocol_out = mul_div(out, protocol, amount_in).unwrap();
    let burn = protocol_out / 2; // burn 2500 : recycle 2500
    assert!(burn > 0);
    assert_eq!(supply(&game), supply_before + out - burn); // the mock minted `out` for its source
    let t = game.treasury();
    assert_eq!(t.reserve, reserve_before + out - burn);
    assert_eq!(t.buyback_sol, 0);
    assert_eq!(game.token_balance(treasury, SOL_MINT, spl_token::ID), 0);
    let (sol, grid) = treasury_books(&game, &[r]);
    assert_eq!((game.lamports(treasury), game.grid_balance(treasury)), (sol, grid)); // surplus used up

    // Creator fees alone: no burn, 100% to the reserve.
    game.svm.airdrop(&treasury, surplus).unwrap();
    let (supply_before, reserve_before) = (supply(&game), game.treasury().reserve);
    let ix = buyback_ix(&mut game, surplus, out);
    game.send(&[ix], WORKER).unwrap();
    assert_eq!(supply(&game), supply_before + out);
    assert_eq!(game.treasury().reserve, reserve_before + out);
    let (sol, grid) = treasury_books(&game, &[r]);
    assert_eq!((game.lamports(treasury), game.grid_balance(treasury)), (sol, grid));
}
