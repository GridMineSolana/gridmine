## Findings

### [Low] Expiry constants extend fund-recycling windows by one third

**Location:** `program/api/src/consts.rs:12–15,40`; `program/program/src/reset.rs:53,109`; `program/program/src/close.rs:22`.

1. A round settles. Reset calculates its expiry using 200 slots per minute.
2. At Solana’s 400 ms target slot time, the advertised one-day window becomes approximately **32 hours**. The 30-day Vault window becomes approximately **40 days**.
3. Close rejects transactions during the additional period. Unclaimed SOL, tokens, and round rent remain locked longer than PLAN specifies. The checkpoint bot’s “last 12 hours” window likewise becomes approximately **16 hours**.

**Minimal fix:** Use 150 slots per minute for target-time approximations, or `Clock.unix_timestamp` for calendar-based deadlines. Test elapsed durations independently of these constants.

Round-one fixes were reviewed. Existing lifecycle, void, refining, RNG, swap-guard, and Vault tests passed using the prebuilt binaries. No files were modified.

## Verdict
VERDICT: THUMBS UP
## Round 2 resolution
- [Low] expiry constants: fixed, `ONE_MINUTE_SLOTS` = 150 (400 ms slots) in `program/api/src/consts.rs`; full suite passes.
