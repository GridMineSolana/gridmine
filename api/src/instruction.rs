use steel::*;

use crate::state::Params;

/// PLAN §6.2: the 20 instructions.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
pub enum GridInstruction {
    // User
    Deploy = 0,
    Automate = 1,
    Checkpoint = 2,
    ClaimSol = 3,
    ClaimToken = 4,
    ClaimVault = 5,
    // Permissionless
    RequestRng = 6,
    ConsumeRng = 7,
    Reset = 8,
    ClaimPot = 9,
    Close = 10,
    FundReserve = 11,
    // Worker
    BuyAsset = 12,
    Buyback = 13,
    // Admin
    Initialize = 14,
    UpdateConfig = 15,
    SetTile = 16,
    SetPause = 17,
    WithdrawTeam = 18,
    SetTokenMint = 19,
}

/// Parses instruction args. Instruction data is not 8-byte aligned after the
/// discriminator, so args are copied out with an unaligned read.
pub fn parse_args<T: Pod>(data: &[u8]) -> Result<T, ProgramError> {
    if data.len() != core::mem::size_of::<T>() {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(bytemuck::pod_read_unaligned(data))
}

/// Parses fixed args followed by pass-through route data.
pub fn parse_args_with_route<T: Pod>(data: &[u8]) -> Result<(T, &[u8]), ProgramError> {
    let n = core::mem::size_of::<T>();
    if data.len() < n {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok((bytemuck::pod_read_unaligned(&data[..n]), &data[n..]))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Deploy {
    /// Lamports per tile.
    pub amount: u64,
    /// Bit i = deploy on tile i.
    pub tiles: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Automate {
    pub amount: u64,
    pub deposit: u64,
    pub fee: u64,
    pub mask: u64,
    pub strategy: u64,
    pub reload: u64,
    pub solo_tiles: u64,
    pub split_tiles: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Checkpoint {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaimSol {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaimToken {
    /// Share of the balance to claim, in bps (10,000 = all).
    pub bps: u64,
}

/// `index`: 0..25 = tile asset, 25 = `$GRID`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaimVault {
    pub round_id: u64,
    pub index: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RequestRng {}

/// Sent by MagicBlock: `[ConsumeRng] ‖ sha256(vrf output) ‖ callback args (round_id)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ConsumeRng {
    pub rng: [u8; 32],
    pub round_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Reset {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClaimPot {
    pub round_id: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Close {}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FundReserve {
    pub amount: u64,
}

pub const BUY_POT: u64 = 0;
pub const BUY_VAULT: u64 = 1;

/// Followed by the Jupiter route instruction data (passed through unchanged).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BuyAsset {
    /// BUY_POT or BUY_VAULT.
    pub kind: u64,
    pub tile: u64,
    pub amount_in: u64,
    pub min_out: u64,
}

/// Followed by the Jupiter route instruction data (passed through unchanged).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Buyback {
    pub amount_in: u64,
    pub min_out: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Initialize {
    pub worker: Pubkey,
    pub admin_collector: Pubkey,
    pub team_collector: Pubkey,
    /// `$GRID` mint (may not exist yet: it is created in the launch tx).
    pub token_mint: Pubkey,
    pub token_program: Pubkey,
    pub token_decimals: u64,
    pub params: Params,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UpdateConfig {
    pub admin: Pubkey,
    pub worker: Pubkey,
    pub admin_collector: Pubkey,
    pub team_collector: Pubkey,
    pub params: Params,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SetTile {
    pub index: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SetPause {
    pub paused: u64,
}

/// No args: the new mint is an account.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SetTokenMint {}

/// `bucket`: 0 = `admin_sol` to admin_collector, 1 = `team_sol` to team_collector.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WithdrawTeam {
    pub amount: u64,
    pub bucket: u64,
}

instruction!(GridInstruction, Deploy);
instruction!(GridInstruction, Automate);
instruction!(GridInstruction, Checkpoint);
instruction!(GridInstruction, ClaimSol);
instruction!(GridInstruction, ClaimToken);
instruction!(GridInstruction, ClaimVault);
instruction!(GridInstruction, RequestRng);
instruction!(GridInstruction, ConsumeRng);
instruction!(GridInstruction, Reset);
instruction!(GridInstruction, ClaimPot);
instruction!(GridInstruction, Close);
instruction!(GridInstruction, BuyAsset);
instruction!(GridInstruction, Buyback);
instruction!(GridInstruction, FundReserve);
instruction!(GridInstruction, Initialize);
instruction!(GridInstruction, UpdateConfig);
instruction!(GridInstruction, SetTile);
instruction!(GridInstruction, SetPause);
instruction!(GridInstruction, WithdrawTeam);
instruction!(GridInstruction, SetTokenMint);
