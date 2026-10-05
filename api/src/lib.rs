pub mod consts;
pub mod error;
pub mod event;
pub mod instruction;
pub mod math;
pub mod sdk;
pub mod state;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::error::*;
    pub use crate::event::*;
    pub use crate::instruction::*;
    pub use crate::math::*;
    pub use crate::sdk::*;
    pub use crate::state::*;
}

use steel::*;

// Mainnet program id.
declare_id!("FAv6eoP4y6wJz2qCTDdKwcLQ2mE93trMi8sZpYdrvpoQ");
