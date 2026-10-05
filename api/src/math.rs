//! Pure money math shared by Reset, Checkpoint and tests.
use solana_program::keccak::hashv;
use steel::ProgramError;

use crate::{consts::*, error::GridError};

pub fn add(a: u64, b: u64) -> Result<u64, ProgramError> {
    a.checked_add(b).ok_or(GridError::Overflow.into())
}

pub fn sub(a: u64, b: u64) -> Result<u64, ProgramError> {
    a.checked_sub(b).ok_or(GridError::Overflow.into())
}

/// floor(a * b / c), in u128. Errors on c == 0 or a u64 overflow.
pub fn mul_div(a: u64, b: u64, c: u64) -> Result<u64, ProgramError> {
    if c == 0 {
        return Err(GridError::Overflow.into());
    }
    u64::try_from(a as u128 * b as u128 / c as u128).map_err(|_| GridError::Overflow.into())
}

/// Splits SOL deployed on one tile into (admin fee, protocol fee, returned).
/// The admin fee applies to every tile; the protocol fee only to losing tiles.
pub fn tile_split(
    deployed: u64,
    is_winner: bool,
    admin_fee_bps: u64,
    protocol_fee_bps: u64,
) -> Result<(u64, u64, u64), ProgramError> {
    let admin = mul_div(deployed, admin_fee_bps, DENOMINATOR_BPS)?;
    let rest = sub(deployed, admin)?;
    let protocol = if is_winner { 0 } else { mul_div(rest, protocol_fee_bps, DENOMINATOR_BPS)? };
    Ok((admin, protocol, sub(rest, protocol)?))
}

/// Protocol fee split into Treasury buckets. `team` takes all rounding dust, so the parts sum to `p`.
#[derive(Debug, PartialEq, Eq)]
pub struct Buckets {
    pub buyback: u64,
    pub pot_per_tile: u64,
    pub vault: u64,
    pub team: u64,
}

pub fn bucket_split(
    p: u64,
    burn_bps: u64,
    recycle_bps: u64,
    pots_bps: u64,
    vault_bps: u64,
) -> Result<Buckets, ProgramError> {
    let buyback = mul_div(p, add(burn_bps, recycle_bps)?, DENOMINATOR_BPS)?;
    let pot_per_tile = mul_div(p, pots_bps, DENOMINATOR_BPS)? / TILES as u64;
    let vault = mul_div(p, vault_bps, DENOMINATOR_BPS)?;
    let team = sub(sub(sub(p, buyback)?, pot_per_tile * TILES as u64)?, vault)?;
    Ok(Buckets { buyback, pot_per_tile, vault, team })
}

/// Per-round emission: `min(reserve * ppb / 1e9 * boost, cap, reserve)`.
pub fn emission(reserve: u64, emission_ppb: u64, boost_bps: u64, cap: u64) -> u64 {
    let e = reserve as u128 * emission_ppb as u128 * boost_bps as u128
        / (1_000_000_000u128 * DENOMINATOR_BPS as u128);
    (e.min(cap as u128) as u64).min(reserve)
}

/// `u128_le(keccak("GRID-RNG-v1" ‖ tag ‖ round_id_le ‖ v)[..16]) % n`.
pub fn draw(v: &[u8; 32], round_id: u64, tag: &[u8], n: u64) -> u64 {
    let h = hashv(&[RNG_DOMAIN, tag, &round_id.to_le_bytes(), v]).0;
    let r = u128::from_le_bytes(h[..16].try_into().unwrap());
    (r % n.max(1) as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMOUNTS: [u64; 9] =
        [0, 1, 99, 100, 101, 5_000_000, 5_000_001, 123_456_789_011, u64::MAX / 10_000];

    /// T1: per-tile split and bucket split add back exactly, for 0, 1 and 25 funded tiles.
    #[test]
    fn t1_fee_math_adds_back_exactly() {
        for (admin_bps, protocol_bps) in [(100, 1000), (0, 0), (500, 2000), (37, 999)] {
            for funded in [0usize, 1, 25] {
                for &amount in &AMOUNTS {
                    for w in [0usize, 24] {
                        let mut totals = (0u128, 0u128, 0u128);
                        for i in 0..TILES {
                            let d = if i < funded { amount.saturating_sub(i as u64) } else { 0 };
                            let (a, p, r) = tile_split(d, i == w, admin_bps, protocol_bps).unwrap();
                            assert_eq!(a as u128 + p as u128 + r as u128, d as u128);
                            if i == w {
                                assert_eq!(p, 0);
                            }
                            totals.0 += a as u128;
                            totals.1 += p as u128;
                            totals.2 += r as u128;
                        }
                        let p = u64::try_from(totals.1).unwrap();
                        let b = bucket_split(p, 2500, 2500, 2000, 2000).unwrap();
                        assert_eq!(b.buyback + b.pot_per_tile * TILES as u64 + b.vault + b.team, p);
                    }
                }
            }
        }
        // Dust: 100 lamports on a losing tile -> 1 admin, 9 protocol, 90 back.
        assert_eq!(tile_split(100, false, 100, 1000).unwrap(), (1, 9, 90));
        assert_eq!(bucket_split(7, 2500, 2500, 2000, 2000).unwrap().team, 7 - 3 - 1);
    }

    /// T2: boost on/off, cap, never more than the reserve.
    #[test]
    fn t2_emission_math() {
        let reserve = 200_000_000 * 1_000_000; // 20% of a 1B, 6-decimal supply
        let base = emission(reserve, 15_000, 10_000, u64::MAX);
        assert_eq!(base, 3_000 * 1_000_000); // ~3,000 $GRID per round on day 1
        assert_eq!(emission(reserve, 15_000, 20_000, u64::MAX), 2 * base);
        assert_eq!(emission(reserve, 15_000, 20_000, 5_000_000_000), 5_000_000_000);
        assert_eq!(emission(100, 1_000_000_000, 30_000, u64::MAX), 100);
        assert_eq!(emission(0, 15_000, 20_000, u64::MAX), 0);
        assert_eq!(emission(u64::MAX, 30_000, 30_000, u64::MAX), (u64::MAX as u128 * 9 / 100_000) as u64);
    }

    #[test]
    fn draw_is_tagged_and_bounded() {
        let v = [42u8; 32];
        assert!(draw(&v, 1, b"tile", 25) < 25);
        let tiles: Vec<u64> = (0..64).map(|id| draw(&v, id, b"tile", 1 << 40)).collect();
        let solos: Vec<u64> = (0..64).map(|id| draw(&v, id, b"solo", 1 << 40)).collect();
        assert_ne!(tiles, solos);
    }
}
