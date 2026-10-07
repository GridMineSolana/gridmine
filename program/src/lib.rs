mod admin;
mod automate;
mod buy_asset;
mod buyback;
mod checkpoint;
mod claim_pot;
mod claim_sol;
mod claim_token;
mod claim_vault;
mod close;
mod deploy;
mod fund_reserve;
mod funds;
mod reset;
mod rng;
mod swap;
mod token;

use admin::*;
use automate::*;
use buy_asset::*;
use buyback::*;
use checkpoint::*;
use claim_pot::*;
use claim_sol::*;
use claim_token::*;
use claim_vault::*;
use close::*;
use deploy::*;
use fund_reserve::*;
use funds::*;
use reset::*;
use rng::*;

use grid_api::instruction::GridInstruction;
use steel::*;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let (ix, data) = parse_instruction(&grid_api::ID, program_id, data)?;

    match ix {
        GridInstruction::Deploy => process_deploy(accounts, data),
        GridInstruction::Automate => process_automate(accounts, data),
        GridInstruction::Checkpoint => process_checkpoint(accounts, data),
        GridInstruction::ClaimSol => process_claim_sol(accounts, data),
        GridInstruction::ClaimToken => process_claim_token(accounts, data),
        GridInstruction::RequestRng => process_request_rng(accounts, data),
        GridInstruction::ConsumeRng => process_consume_rng(accounts, data),
        GridInstruction::Reset => process_reset(accounts, data),
        GridInstruction::Close => process_close(accounts, data),
        GridInstruction::FundReserve => process_fund_reserve(accounts, data),
        GridInstruction::Initialize => process_initialize(accounts, data),
        GridInstruction::UpdateConfig => process_update_config(accounts, data),
        GridInstruction::SetTile => process_set_tile(accounts, data),
        GridInstruction::SetPause => process_set_pause(accounts, data),
        GridInstruction::WithdrawTeam => process_withdraw_team(accounts, data),
        GridInstruction::SetTokenMint => process_set_token_mint(accounts, data),
        GridInstruction::ClaimVault => process_claim_vault(accounts, data),
        GridInstruction::ClaimPot => process_claim_pot(accounts, data),
        GridInstruction::BuyAsset => process_buy_asset(accounts, data),
        GridInstruction::Buyback => process_buyback(accounts, data),
        GridInstruction::Fund => process_fund(accounts, data),
        GridInstruction::Withdraw => process_withdraw(accounts, data),
    }
}

entrypoint!(process_instruction);

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "GRIDMINE",
    project_url: "https://gridmine.fun",
    contacts: "email:gridminesol@gmail.com",
    policy: "https://github.com/GridMineSolana/gridmine/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/GridMineSolana/gridmine",
    auditors: "Internal review; reports in repo"
}
