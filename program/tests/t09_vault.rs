//! T9: forced Vault hit, three tickets, ClaimVault per mint pro-rata (sum <= vault), double claims
//! fail, a frozen Token-2022 Treasury ATA blocks only its own mint, leftovers return on Close.
mod common;

use common::*;
use grid_api::prelude::*;
use solana_sdk::pubkey::Pubkey;

const RESERVE: u64 = 200_000_000 * 1_000_000;
const T22: Pubkey = spl_token_2022::ID;

fn claim(game: &mut Game, miner: Pubkey, r: u64, i: usize) -> TxResult {
    let (mint, program) = if i == VAULT_GRID {
        (game.token_mint, T22)
    } else {
        let t = game.config().tiles[i];
        (t.mint, t.token_program())
    };
    game.send(&[claim_vault(miner, r, i as u64, mint, program)], ADMIN) // ADMIN pays the tx fee
}

#[test]
fn t9_vault_hit_claims_close() {
    let mut game = Game::ready();
    game.fund_reserve(RESERVE);
    let xstock = game.create_mint(true, 8, Ext::None);
    game.send(&[set_tile(ADMIN, 7, xstock)], ADMIN).unwrap();

    // Fill the Vault: losing SOL, then BuyAsset(vault) into tiles 6 (SPL) and 7 (Token-2022).
    let filler = game.user();
    let r0 = game.play_round(&[(filler, 10 * SOL, tile(0) | tile(1))], 0);
    game.checkpoint(filler, r0).unwrap();
    let v = game.treasury().vault_sol;
    game.buy(BUY_VAULT, 6, v / 2, 3_000_000, 0).unwrap();
    game.buy(BUY_VAULT, 7, v / 2, 7_000_001, 0).unwrap();
    let t = game.treasury();
    assert_eq!((t.vault[6], t.vault[7], t.vault_spent[6], t.vault_sol), (3_000_000, 7_000_001, v / 2, v - v / 2 * 2));

    // Forced hit: a, b, c share winning tile 9; d loses.
    game.set_params(Params { vault_odds: 1, ..default_params() }).unwrap();
    let (a, b, c, d) = (game.user(), game.user(), game.user(), game.user());
    let r = game.board().round_id;
    for (m, amount, t) in [(a, 10_000_000, 9), (b, 20_000_000, 9), (c, 30_000_000, 9), (d, 10_000_000, 10)] {
        game.deploy(m, amount, tile(t)).unwrap();
    }
    let end = game.board().end_slot;
    game.vrf_tile(r, 9);
    let vault_before = game.treasury().vault;
    game.reset().unwrap();
    let round = game.round(r);
    assert_eq!(round.vault_hit, 1);
    assert_eq!(round.expires_at, end + default_params().intermission_slots + VAULT_EXPIRY_SLOTS); // from Reset
    assert_eq!((round.vault[6], round.vault[7]), (3_000_000, 7_000_001));
    assert!(round.vault[VAULT_GRID] > vault_before[VAULT_GRID]); // includes this round's emission
    assert_eq!(game.treasury().vault, [0; 26]);
    assert_eq!((game.treasury().owed[6], game.treasury().owed[7]), (3_000_000, 7_000_001));

    // Checkpoint (by a bot): tickets for winning-tile miners, rent out of their winnings.
    let rent = game.svm.minimum_balance_for_rent_exemption(VaultTicket::SIZE);
    let (_, _, back) = tile_split(round.deployed[9], true, 100, 1000).unwrap();
    let open = (1 << 6) | (1 << 7) | (1 << VAULT_GRID);
    for (m, amount) in [(a, 10_000_000), (b, 20_000_000), (c, 30_000_000)] {
        let before = game.lamports(m);
        game.send(&[checkpoint(ADMIN, m, r)], ADMIN).unwrap();
        assert_eq!(game.lamports(m), before + mul_div(amount, back, round.deployed[9]).unwrap() - rent);
        let ticket: VaultTicket = game.read(ticket_pda(r, m).0);
        assert_eq!((ticket.authority, ticket.share, ticket.claimed), (m, amount, ALL_CLAIMED & !open));
    }
    game.checkpoint(d, r).unwrap();
    assert!(game.svm.get_account(&ticket_pda(r, d).0).is_none_or(|t| t.data.is_empty()));

    // Pro-rata per mint; double claims and empty entries fail; d has no ticket.
    claim(&mut game, a, r, 6).unwrap();
    assert_eq!(game.token_balance(a, game.config().tiles[6].mint, spl_token::ID), 500_000);
    assert_eq!(error_code(claim(&mut game, a, r, 6)), GridError::AlreadyClaimed as u32);
    assert_eq!(error_code(claim(&mut game, a, r, 5)), GridError::AlreadyClaimed as u32);
    assert!(claim(&mut game, d, r, 6).is_err());

    // The issuer freezes the Treasury's tile-7 ATA: only tile 7 claims fail.
    let treasury = treasury_pda().0;
    let frozen = ata(&treasury, &xstock, &T22);
    game.send(&[spl_token_2022::instruction::freeze_account(&T22, &frozen, &xstock, &ADMIN, &[]).unwrap()], ADMIN).unwrap();
    assert!(claim(&mut game, b, r, 7).is_err());
    claim(&mut game, b, r, 6).unwrap();
    claim(&mut game, b, r, VAULT_GRID).unwrap();
    game.send(&[spl_token_2022::instruction::thaw_account(&T22, &frozen, &xstock, &ADMIN, &[]).unwrap()], ADMIN).unwrap();

    // b claims the last entry: ticket closed, rent back (ATA made first so its rent doesn't count).
    game.mint_to(b, xstock, T22, 0);
    let before = game.lamports(b);
    claim(&mut game, b, r, 7).unwrap();
    assert_eq!(game.token_balance(b, xstock, T22), 7_000_001 * 20 / 60);
    assert!(game.svm.get_account(&ticket_pda(r, b).0).is_none_or(|t| t.data.is_empty()));
    assert_eq!(game.lamports(b), before + rent);
    claim(&mut game, a, r, 7).unwrap();
    claim(&mut game, a, r, VAULT_GRID).unwrap();
    let paid = game.round(r).vault_paid;
    for i in [6, 7, VAULT_GRID] {
        assert!(paid[i] <= round.vault[i]);
    }
    game.assert_tile_books(6);
    game.assert_tile_books(7);
    let (sol, grid) = treasury_books(&game, &[r]);
    assert_eq!((game.lamports(treasury), game.grid_balance(treasury)), (sol, grid));

    // c never claims. After 30 days Close returns its shares to the Vault.
    game.warp_to(round.expires_at + 1);
    game.send(&[close(ADMIN, r, round.rent_payer)], ADMIN).unwrap();
    let t = game.treasury();
    for i in [6, 7, VAULT_GRID] {
        assert_eq!(t.vault[i], round.vault[i] - paid[i]);
    }
    assert_eq!((t.owed[6], t.owed[7]), (0, 0));
    game.assert_tile_books(6);
    game.assert_tile_books(7);
    let (sol, grid) = treasury_books(&game, &[]);
    assert_eq!((game.lamports(treasury), game.grid_balance(treasury)), (sol, grid));

    // c's ticket can only be closed now (rent back, no tokens).
    let before = game.lamports(c);
    claim(&mut game, c, r, 6).unwrap();
    assert_eq!(game.lamports(c), before + rent);
    assert_eq!(game.token_balance(c, game.config().tiles[6].mint, spl_token::ID), 0);
}
