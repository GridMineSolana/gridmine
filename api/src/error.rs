use steel::*;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum GridError {
    #[error("Amount too small")]
    AmountTooSmall = 0,
    #[error("Not authorized")]
    NotAuthorized = 1,
    #[error("Invalid executor")]
    InvalidExecutor = 2,
    #[error("Deploys are paused")]
    Paused = 3,
    #[error("Round is not accepting deploys")]
    RoundNotActive = 4,
    #[error("Round cannot be reset yet")]
    RoundNotOver = 5,
    #[error("Randomness not ready")]
    RngNotReady = 6,
    #[error("Miner has not checkpointed")]
    NotCheckpointed = 7,
    #[error("Invalid automation strategy")]
    InvalidStrategy = 8,
    #[error("Invalid config")]
    InvalidConfig = 9,
    #[error("Invalid tile")]
    InvalidTile = 10,
    #[error("Unsupported mint")]
    UnsupportedMint = 11,
    #[error("Tile still holds pot or vault tokens")]
    TileNotEmpty = 12,
    #[error("Math overflow")]
    Overflow = 13,
    #[error("Insufficient funds")]
    InsufficientFunds = 14,
    #[error("Round id mismatch")]
    InvalidRound = 15,
    #[error("Randomness already requested or set")]
    RngRequested = 16,
    #[error("Swap over the cap or too soon")]
    SwapLimit = 17,
    #[error("Swap returned less than min_out")]
    MinOut = 18,
    #[error("Swap route touched a forbidden account")]
    BadRoute = 19,
    #[error("Already claimed")]
    AlreadyClaimed = 20,
    #[error("Nothing to claim")]
    NothingToClaim = 21,
}

error!(GridError);
