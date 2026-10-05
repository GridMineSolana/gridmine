use steel::*;

/// Events are emitted with `sol_log_data` (steel `Loggable::log`). `disc` identifies the type.
pub enum GridEvent {
    Reset = 0,
    Deploy = 2,
    Claim = 4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ResetEvent {
    pub disc: u64,
    pub round_id: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    /// u64::MAX on a void round.
    pub winning_tile: u64,
    pub top_miner: Pubkey,
    pub total_miners: u64,
    pub total_deployed: u64,
    pub admin_fee: u64,
    pub protocol_fee: u64,
    pub total_returned: u64,
    /// Total `$GRID` emitted (token_reward + vault part).
    pub emission: u64,
    pub token_reward: u64,
    pub deployed_winning_tile: u64,
    pub ts: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct DeployEvent {
    pub disc: u64,
    pub authority: Pubkey,
    pub amount: u64,
    pub mask: u64,
    pub round_id: u64,
    pub signer: Pubkey,
    /// Automation strategy, u64::MAX if manual.
    pub strategy: u64,
    pub total_tiles: u64,
    pub ts: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct ClaimEvent {
    pub disc: u64,
    pub authority: Pubkey,
    pub amount: u64,
    /// 0 = SOL, 1 = `$GRID`.
    pub claim_type: u64,
    pub ts: i64,
}

event!(ResetEvent);
event!(DeployEvent);
event!(ClaimEvent);
