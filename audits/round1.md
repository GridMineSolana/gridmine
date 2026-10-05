## Findings

### [High] Automate-created miners lose their round-zero deposits

**Location:** `program/program/src/automate.rs:35–46`; `program/program/src/checkpoint.rs:31–32`.

1. A new user creates an automation before their first deployment.
2. `Automate` leaves `round_id` and `checkpoint_id` at zero, unlike `Deploy`, which initializes both to `u64::MAX`.
3. The automation deposits into round zero. After settlement, every `Checkpoint` returns immediately because both IDs equal zero.
4. The user receives no SOL return, emission, pot selection, or Vault ticket. `Close` eventually recycles the unpaid funds.

**Verified:** An in-memory LiteSVM reproduction deposited 1 SOL through automation into round zero. After voiding and checkpointing, the refund was zero and the round retained the full SOL.

**Minimal fix:** Initialize both IDs to `u64::MAX` when `Automate` creates a Miner. Add round-zero automation tests for normal settlement and refunds.

### [High] Delayed settlement can forfeit deposits before refunds become available

**Location:** `program/program/src/deploy.rs:45`; `program/program/src/reset.rs:123–126`; `program/program/src/checkpoint.rs:48–51`; `program/program/src/close.rs:29–30`.

1. Users deposit, but no successful `Reset` occurs before `end_slot + ONE_DAY_SLOTS`.
2. Checkpointing before Reset pays nothing because the round remains current.
3. A later Reset voids the round, but retains its already-expired claim deadline.
4. Checkpoint now marks deposits forfeited. Anyone can immediately Close and move all refundable SOL into `buyback_sol`.

Thus a prolonged crank outage can cause complete deposit loss, rather than merely delaying refunds.

**Verified:** Delaying Reset past expiry produced a void round with zero refund; Close credited the deposited 1 SOL to `buyback_sol`.

**Minimal fix:** Set the claim deadline at settlement, giving normal and void rounds a full claim window after Reset. Apply the same protection to Vault-hit rounds.

### [Medium] Precreating a wSOL ATA permanently blocks Initialize

**Location:** `program/program/src/admin.rs:62–72`.

1. Before initialization, an attacker creates the canonical wSOL ATA for either the Treasury PDA or swap PDA. ATA creation requires no owner signature.
2. Initialize unconditionally calls Steel’s non-idempotent ATA creation helper.
3. The existing ATA causes `IllegalOwner`, reverting initialization.
4. Retrying has the same result; the attacker need not maintain the attack.

The initialization authority check prevents takeover but does not prevent this launch-blocking attack.

**Verified:** Precreating the swap PDA’s ATA caused Initialize to fail with `IllegalOwner`.

**Minimal fix:** Use idempotent ATA creation and validate each existing account’s token program, mint, and authority.

### [Medium] Anyone can consume another miner’s deployment slots

**Location:** `program/program/src/deploy.rs:48–61,105–107,136–138,170–171`.

1. Choose an existing, checkpointed Miner without an automation.
2. Submit Deploy with the attacker as signer and the victim as authority.
3. The program accepts the deployment and charges the attacker, without requiring the victim’s consent.
4. Because each miner may deploy only once per tile, the victim’s subsequent deployment silently skips those tiles.

An attacker can suppress a competitor’s intended stake and winning probability by front-running with minimum-sized deposits.

**Verified:** An attacker’s 0.005 SOL deployment prevented the victim’s subsequent 10 SOL deployment on that tile. The second instruction succeeded but added zero lamports.

**Minimal fix:** Require `signer_info.key == authority_info.key` when no automation exists. Retain executor authorization for automated deployments.

### [Medium] Late randomness overrides the promised void deadline

**Location:** `program/program/src/rng.rs:101–106`; `program/program/src/reset.rs:52,123`.

1. No randomness arrives before `end_slot + void_after_slots`.
2. Before anyone executes Reset, a late VRF callback arrives.
3. ConsumeRng accepts it without checking a deadline.
4. Reset prioritizes any stored randomness over timeout, charges fees, and settles normally instead of refunding.

After timeout, callback-versus-Reset ordering determines whether users receive refunds or a game outcome. This violates the stated deadline and introduces an avoidable settlement race.

**Verified:** Delivering randomness one slot after the default void deadline resulted in `void = 0` and fees being charged.

**Minimal fix:** Snapshot a randomness deadline in each Round. Ignore callbacks after that deadline; Reset should settle only with randomness accepted before it.

### [Low] Zero-tile deployments start empty rounds

**Location:** `program/program/src/deploy.rs:41–45,67–87,132–172`.

1. Submit Deploy with a valid amount but `tiles = 0`.
2. The timer starts before the program determines whether any SOL will be deployed.
3. The instruction succeeds with zero deposits.
4. Repeating this after each Reset can make a normal worker fund VRF requests and advance empty rounds without mining participation.

**Verified:** A zero-mask deployment left every tile’s deposited amount at zero while setting a finite `end_slot`.

**Minimal fix:** Require at least one effective tile deployment before starting the timer. Reject masks with no valid tile bits.

## Verdict
VERDICT: CHANGES REQUIRED

## Round 1 resolution
- [High] Automate round-zero deposits: fixed. `program/program/src/automate.rs` starts new miners at round_id = checkpoint_id = u64::MAX. Test `t05_void.rs::t5_automation_round_zero_refund`.
- [High] Delayed settlement forfeits deposits: fixed. `program/program/src/reset.rs` sets expires_at = Reset slot + 1 day (hit rounds + 30 days); `deploy.rs` no longer sets it. Test `t05_void.rs::t5_late_reset_still_refunds` (also `t09_vault.rs` expiry assertion).
- [Medium] Precreated wSOL ATA blocks Initialize: fixed. `program/program/src/admin.rs` uses `ensure_ata` (create if empty, else check canonical ATA address + token account). Test `t04_lifecycle.rs::t4_initialize_with_precreated_wsol_atas`.
- [Medium] Deploy without the authority's consent: fixed. `program/program/src/deploy.rs` requires signer == authority when no automation exists. Test `t04_lifecycle.rs::t4_deploy_guards`.
- [Medium] Late randomness overrides the void deadline: fixed. New `Round.rng_deadline` (end + void_after_slots, set by the first deposit); `rng.rs` ConsumeRng ignores values from that slot on and `reset.rs` voids on it. Test `t05_void.rs::t5_void_refunds_everything` (answer at the deadline is ignored).
- [Low] Zero-tile deploys start empty rounds: fixed. `program/program/src/deploy.rs` starts the timer only when the deploy moved SOL. Covered by `t04_lifecycle.rs::t4_deploy_guards`.
