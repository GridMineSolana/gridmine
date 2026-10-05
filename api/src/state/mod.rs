mod automation;
mod board;
mod config;
mod miner;
mod round;
mod treasury;
mod vault_ticket;

pub use automation::*;
pub use board::*;
pub use config::*;
pub use miner::*;
pub use round::*;
pub use treasury::*;
pub use vault_ticket::*;

use crate::consts::*;

use steel::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum GridAccount {
    Automation = 100,
    Config = 101,
    Miner = 103,
    Treasury = 104,
    Board = 105,
    Round = 109,
    VaultTicket = 110,
}

pub fn automation_pda(authority: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[AUTOMATION, &authority.to_bytes()], &crate::ID)
}

pub fn board_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BOARD], &crate::ID)
}

pub fn config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG], &crate::ID)
}

pub fn miner_pda(authority: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[MINER, &authority.to_bytes()], &crate::ID)
}

pub fn round_pda(id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ROUND, &id.to_le_bytes()], &crate::ID)
}

pub fn treasury_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TREASURY], &crate::ID)
}

pub fn ticket_pda(round_id: u64, authority: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[TICKET, &round_id.to_le_bytes(), &authority.to_bytes()],
        &crate::ID,
    )
}

pub fn swap_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SWAP], &crate::ID)
}

pub fn identity_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[IDENTITY], &crate::ID)
}

/// MagicBlock's scoped VRF identity for this program. It signs every ConsumeRng callback.
pub fn vrf_identity() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[IDENTITY, &crate::ID.to_bytes()], &VRF_PROGRAM_ID)
}

/// Associated token address for `owner`, using the given token program.
pub fn ata(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    spl_associated_token_account::get_associated_token_address_with_program_id(
        owner,
        mint,
        token_program,
    )
}
