//! Instruction builders. Account order matches each processor's destructuring.
use solana_program::pubkey::Pubkey;
use steel::*;

use solana_program::sysvar::slot_hashes;

use crate::{consts::*, instruction::*, state::*};

fn ix(accounts: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction { program_id: crate::ID, accounts, data }
}

pub fn initialize(signer: Pubkey, args: Initialize) -> Instruction {
    let treasury = treasury_pda().0;
    let swap = swap_pda().0;
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config_pda().0, false),
            AccountMeta::new(board_pda().0, false),
            AccountMeta::new(round_pda(0).0, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new(ata(&treasury, &SOL_MINT, &spl_token::ID), false),
            AccountMeta::new_readonly(swap, false),
            AccountMeta::new(ata(&swap, &SOL_MINT, &spl_token::ID), false),
            AccountMeta::new_readonly(SOL_MINT, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        args.to_bytes(),
    )
}

pub fn update_config(signer: Pubkey, args: UpdateConfig) -> Instruction {
    ix(
        vec![AccountMeta::new(signer, true), AccountMeta::new(config_pda().0, false)],
        args.to_bytes(),
    )
}

pub fn set_tile(signer: Pubkey, index: u64, mint: Pubkey) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config_pda().0, false),
            AccountMeta::new_readonly(treasury_pda().0, false),
            AccountMeta::new_readonly(mint, false),
        ],
        SetTile { index }.to_bytes(),
    )
}

pub fn set_pause(signer: Pubkey, paused: bool) -> Instruction {
    ix(
        vec![AccountMeta::new(signer, true), AccountMeta::new(config_pda().0, false)],
        SetPause { paused: paused as u64 }.to_bytes(),
    )
}

pub fn deploy(signer: Pubkey, authority: Pubkey, amount: u64, round_id: u64, tiles: u64) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(authority, false),
            AccountMeta::new(automation_pda(authority).0, false),
            AccountMeta::new(board_pda().0, false),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(miner_pda(authority).0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        Deploy { amount, tiles }.to_bytes(),
    )
}

/// `executor == Pubkey::default()` closes the automation.
pub fn automate(signer: Pubkey, executor: Pubkey, args: Automate) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(automation_pda(signer).0, false),
            AccountMeta::new_readonly(executor, false),
            AccountMeta::new(miner_pda(signer).0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        args.to_bytes(),
    )
}

pub fn checkpoint(signer: Pubkey, authority: Pubkey, round_id: u64) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(authority, false),
            AccountMeta::new(automation_pda(authority).0, false),
            AccountMeta::new_readonly(board_pda().0, false),
            AccountMeta::new(miner_pda(authority).0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new(treasury_pda().0, false),
            AccountMeta::new(ticket_pda(round_id, authority).0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        Checkpoint {}.to_bytes(),
    )
}

pub fn claim_sol(signer: Pubkey) -> Instruction {
    ix(
        vec![AccountMeta::new(signer, true), AccountMeta::new(miner_pda(signer).0, false)],
        ClaimSol {}.to_bytes(),
    )
}

pub fn claim_token(signer: Pubkey, mint: Pubkey, token_program: Pubkey, bps: u64) -> Instruction {
    let treasury = treasury_pda().0;
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(miner_pda(signer).0, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(ata(&signer, &mint, &token_program), false),
            AccountMeta::new(treasury, false),
            AccountMeta::new(ata(&treasury, &mint, &token_program), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        ClaimToken { bps }.to_bytes(),
    )
}

pub fn withdraw_team(signer: Pubkey, collector: Pubkey, amount: u64, bucket: u64) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(config_pda().0, false),
            AccountMeta::new(treasury_pda().0, false),
            AccountMeta::new(collector, false),
        ],
        WithdrawTeam { amount, bucket }.to_bytes(),
    )
}

/// `signer` pays the VRF fee.
pub fn request_rng(signer: Pubkey, round_id: u64) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(board_pda().0, false),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new_readonly(identity_pda().0, false),
            AccountMeta::new(VRF_QUEUE, false),
            AccountMeta::new_readonly(VRF_PROGRAM_ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(slot_hashes::ID, false),
        ],
        RequestRng {}.to_bytes(),
    )
}

/// The callback MagicBlock sends (`signer` must be `vrf_identity()`). Used by tests.
pub fn consume_rng(signer: Pubkey, round_id: u64, rng: [u8; 32]) -> Instruction {
    ix(
        vec![AccountMeta::new_readonly(signer, true), AccountMeta::new(round_pda(round_id).0, false)],
        ConsumeRng { rng, round_id }.to_bytes(),
    )
}

pub fn reset(signer: Pubkey, round_id: u64) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new(board_pda().0, false),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new(round_pda(round_id + 1).0, false),
            AccountMeta::new(treasury_pda().0, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        Reset {}.to_bytes(),
    )
}

pub fn close(signer: Pubkey, round_id: u64, rent_payer: Pubkey) -> Instruction {
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(board_pda().0, false),
            AccountMeta::new(rent_payer, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new(treasury_pda().0, false),
        ],
        Close {}.to_bytes(),
    )
}

pub fn fund_reserve(signer: Pubkey, mint: Pubkey, token_program: Pubkey, amount: u64) -> Instruction {
    let treasury = treasury_pda().0;
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(ata(&signer, &mint, &token_program), false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new(ata(&treasury, &mint, &token_program), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        FundReserve { amount }.to_bytes(),
    )
}

/// Pays the pot of `round_id` to `winner` (round.pot_winner). `signer` pays the winner's ATA rent.
pub fn claim_pot(signer: Pubkey, round_id: u64, winner: Pubkey, mint: Pubkey, token_program: Pubkey) -> Instruction {
    let treasury = treasury_pda().0;
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(ata(&treasury, &mint, &token_program), false),
            AccountMeta::new_readonly(winner, false),
            AccountMeta::new(ata(&winner, &mint, &token_program), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        ClaimPot { round_id }.to_bytes(),
    )
}

/// Claims vault entry `index` (25 = `$GRID`) of `round_id` for `signer`.
pub fn claim_vault(signer: Pubkey, round_id: u64, index: u64, mint: Pubkey, token_program: Pubkey) -> Instruction {
    let treasury = treasury_pda().0;
    ix(
        vec![
            AccountMeta::new(signer, true),
            AccountMeta::new_readonly(config_pda().0, false),
            AccountMeta::new(round_pda(round_id).0, false),
            AccountMeta::new(ticket_pda(round_id, signer).0, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(ata(&treasury, &mint, &token_program), false),
            AccountMeta::new(ata(&signer, &mint, &token_program), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
        ],
        ClaimVault { round_id, index }.to_bytes(),
    )
}

/// Swaps `amount_in` of a pot or vault bucket into tile `tile` (Treasury's ATA for `mint` must exist).
/// `route` is Jupiter's swap instruction built with taker = swap PDA, source = swap wSOL ATA.
#[allow(clippy::too_many_arguments)]
pub fn buy_asset(
    signer: Pubkey,
    kind: u64,
    tile: u64,
    amount_in: u64,
    min_out: u64,
    mint: Pubkey,
    token_program: Pubkey,
    route: Instruction,
) -> Instruction {
    let swap = swap_pda().0;
    let mut accounts = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new(config_pda().0, false),
        AccountMeta::new(treasury_pda().0, false),
        AccountMeta::new_readonly(swap, false),
        AccountMeta::new(ata(&swap, &SOL_MINT, &spl_token::ID), false),
        AccountMeta::new(ata(&treasury_pda().0, &mint, &token_program), false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(route.program_id, false),
    ];
    accounts.extend(route.accounts.into_iter().map(|a| AccountMeta { is_signer: false, ..a }));
    let mut data = BuyAsset { kind, tile, amount_in, min_out }.to_bytes();
    data.extend(route.data);
    ix(accounts, data)
}

/// Swaps `amount_in` of the buyback bucket plus creator fees into `$GRID`, then burns/recycles.
pub fn buyback(signer: Pubkey, mint: Pubkey, token_program: Pubkey, amount_in: u64, min_out: u64, route: Instruction) -> Instruction {
    let (swap, treasury) = (swap_pda().0, treasury_pda().0);
    let mut accounts = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new(config_pda().0, false),
        AccountMeta::new(treasury, false),
        AccountMeta::new(ata(&treasury, &SOL_MINT, &spl_token::ID), false),
        AccountMeta::new_readonly(swap, false),
        AccountMeta::new(ata(&swap, &SOL_MINT, &spl_token::ID), false),
        AccountMeta::new(mint, false),
        AccountMeta::new(ata(&treasury, &mint, &token_program), false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(route.program_id, false),
    ];
    accounts.extend(route.accounts.into_iter().map(|a| AccountMeta { is_signer: false, ..a }));
    let mut data = Buyback { amount_in, min_out }.to_bytes();
    data.extend(route.data);
    ix(accounts, data)
}
