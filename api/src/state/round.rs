use solana_program::keccak;
use steel::*;

use crate::{consts::*, state::GridAccount};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Round {
    pub id: u64,
    /// SOL deployed on each tile.
    pub deployed: [u64; TILES],
    /// Unique miners on each tile.
    pub count: [u64; TILES],
    /// VRF value. All zero until ConsumeRng stores it.
    pub rng: [u8; 32],
    pub rng_requested_slot: u64,
    /// `end_slot + void_after_slots`, set by the first deploy. ConsumeRng ignores values from
    /// this slot on; Reset voids the round if none arrived before it.
    pub rng_deadline: u64,
    /// Slot after which Close may run. u64::MAX until Reset (claim window starts at Reset).
    pub expires_at: u64,
    /// Receives this account's rent on Close.
    pub rent_payer: Pubkey,
    /// SPLIT_ADDRESS when the `$GRID` reward is split, else the solo winner once checkpointed.
    pub top_miner: Pubkey,
    /// u64::MAX until Reset.
    pub winning_tile: u64,
    /// 1 = no randomness arrived in time; everyone is refunded 100%.
    pub void: u64,
    /// Fee bps snapshot taken at Reset, so Checkpoint returns match Reset's fees.
    pub admin_fee_bps: u64,
    pub protocol_fee_bps: u64,
    pub admin_fee: u64,
    pub protocol_fee: u64,
    pub total_returned_sol: u64,
    pub total_miners: u64,
    /// `$GRID` for the winning tile (5/6 of the emission).
    pub token_reward: u64,
    /// `$GRID` credited to miners so far. The rest returns to the reserve on Close.
    pub token_credited: u64,
    /// Winning tile's pot tokens handed to this round.
    pub pot_amount: u64,
    /// Set by the Checkpoint of the miner that `draw(rng, "pot")` lands on.
    pub pot_winner: Pubkey,
    /// Pot tokens paid by ClaimPot (0 or pot_amount).
    pub pot_paid: u64,
    /// 1 = the Vault hit this round.
    pub vault_hit: u64,
    /// Vault snapshot on a hit (0..25 tiles, 25 = `$GRID`).
    pub vault: [u64; 26],
    pub vault_paid: [u64; 26],
}

impl Round {
    pub fn total_deployed(&self) -> u64 {
        self.deployed.iter().sum()
    }

    /// True if the `$GRID` reward on `tile` is split pro-rata (ORE's public mask: 15 split, 10 solo).
    pub fn is_split_reward(&self, tile: usize) -> bool {
        self.distribution_mask() & (1 << tile) == 0
    }

    /// 25-bit mask, bit set = solo tile. Deterministic from the round id (ORE).
    pub fn distribution_mask(&self) -> u32 {
        const BITS: usize = 10;
        let mut randomness = keccak::hashv(&[self.id.to_le_bytes().as_ref()]).0;
        let mut indices: [u8; TILES] = core::array::from_fn(|i| i as u8);
        let mut offset = 0;
        for i in (1..TILES).rev() {
            if offset + 2 > randomness.len() {
                randomness = keccak::hashv(&[&randomness]).0;
                offset = 0;
            }
            let r = u16::from_le_bytes([randomness[offset], randomness[offset + 1]]);
            indices.swap(i, r as usize % (i + 1));
            offset += 2;
        }
        indices[..BITS].iter().fold(0, |mask, &idx| mask | 1 << idx)
    }
}

account!(GridAccount, Round);

#[cfg(test)]
mod tests {
    use super::*;

    fn round(id: u64) -> Round {
        Round { id, ..Zeroable::zeroed() }
    }

    #[test]
    fn distribution_mask_has_10_of_25_bits_and_varies() {
        let mut masks = std::collections::HashSet::new();
        for id in 0..1000 {
            let mask = round(id).distribution_mask();
            assert_eq!(mask.count_ones(), 10);
            assert_eq!(mask >> 25, 0);
            masks.insert(mask);
        }
        assert!(masks.len() > 500);
    }
}
