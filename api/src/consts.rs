use solana_program::{pubkey, pubkey::Pubkey};

/// The only key allowed to call `Initialize`.
pub const INIT_AUTHORITY: Pubkey = pubkey!("33pZ84VbUy31NQdaGV6bqDXvqtFN5LprMYTWWJM7p8bv");

/// Number of tiles on the board.
pub const TILES: usize = 25;

/// Index of `$GRID` in the 26-entry vault arrays (0..25 are tiles).
pub const VAULT_GRID: usize = 25;

pub const ONE_MINUTE_SLOTS: u64 = 150; // 400 ms target slot time
pub const ONE_HOUR_SLOTS: u64 = 60 * ONE_MINUTE_SLOTS;
pub const TWELVE_HOURS_SLOTS: u64 = 12 * ONE_HOUR_SLOTS;
pub const ONE_DAY_SLOTS: u64 = 24 * ONE_HOUR_SLOTS;

/// PDA seeds.
pub const AUTOMATION: &[u8] = b"automation";
pub const BOARD: &[u8] = b"board";
pub const CONFIG: &[u8] = b"config";
pub const MINER: &[u8] = b"miner";
pub const ROUND: &[u8] = b"round";
pub const TREASURY: &[u8] = b"treasury";
pub const TICKET: &[u8] = b"ticket";
pub const SWAP: &[u8] = b"swap";
pub const IDENTITY: &[u8] = b"identity";

/// MagicBlock VRF program and its default (L1) queue. Verified against `ephemeral-vrf-sdk` 0.17.3
/// `consts.rs` and `magicblock-labs/solana-vrf` @ 52b103c.
pub const VRF_PROGRAM_ID: Pubkey = pubkey!("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
pub const VRF_QUEUE: Pubkey = pubkey!("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");

/// Domain tag for `draw`.
pub const RNG_DOMAIN: &[u8] = b"GRID-RNG-v1";

/// Jupiter Aggregator v6. The only program BuyAsset and Buyback may CPI with the swap PDA signing.
pub const JUPITER_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// A vault-hit round lives this long (PLAN §2 simplifications).
pub const VAULT_EXPIRY_SLOTS: u64 = 30 * ONE_DAY_SLOTS;

/// VaultTicket.claimed when all 26 entries are claimed.
pub const ALL_CLAIMED: u64 = (1 << 26) - 1;

pub const SOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// Marks a round whose `$GRID` reward is split pro-rata.
pub const SPLIT_ADDRESS: Pubkey = pubkey!("SpLiT11111111111111111111111111111111111112");

/// Marks an automation anyone may execute.
pub const EXECUTOR_ADDRESS: Pubkey = pubkey!("executor11111111111111111111111111111111112");

pub const DENOMINATOR_BPS: u64 = 10_000;

/// Paid to whoever checkpoints a miner in the last 12h before round expiry.
pub const CHECKPOINT_FEE: u64 = 10_000;
