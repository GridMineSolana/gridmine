//! Shared LiteSVM helpers. Signature checks are off, so any pubkey can sign; tests only
//! exercise program logic. Load order: `cargo build-sbf` first (tests read target/deploy/grid.so).
#![allow(dead_code)]

use grid_api::prelude::*;
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_sdk::{
    account::Account, instruction::InstructionError, message::Message, pubkey::Pubkey,
    system_instruction, transaction::Transaction, transaction::TransactionError,
};
use solana_sdk::program_pack::Pack;
use spl_token_2022::extension::ExtensionType;
use steel::{AccountMeta, Instruction, Pod};

pub const ADMIN: Pubkey = INIT_AUTHORITY;
pub const COLLECTOR: Pubkey = Pubkey::new_from_array([7; 32]);
pub const TEAM: Pubkey = Pubkey::new_from_array([8; 32]);
pub const WORKER: Pubkey = Pubkey::new_from_array([9; 32]);
pub const SOL: u64 = 1_000_000_000;
/// `$GRID` decimals (pump.fun Token-2022 mint).
pub const GRID_DECIMALS: u8 = 6;

pub type TxResult = Result<(), Box<FailedTransactionMetadata>>;

/// PLAN §4 values.
pub fn default_params() -> Params {
    Params {
        round_slots: 240,
        intermission_slots: 48,
        min_deploy: 5_000_000,
        admin_fee_bps: 100,
        protocol_fee_bps: 1000,
        burn_bps: 2500,
        recycle_bps: 2500,
        pots_bps: 2000,
        vault_bps: 2000,
        team_bps: 1000,
        emission_ppb: 15_000,
        max_emission: 10_000 * 10u64.pow(GRID_DECIMALS as u32),
        boost_bps: 20_000,
        boost_until: 14 * 86_400, // LiteSVM clock starts at unix 0
        vault_emission_bps: 1667,
        vault_odds: 1000,
        refine_fee_bps: 1000,
        void_after_slots: 1500,
        rerequest_after_slots: 300,
        max_swap_lamports: 5 * SOL,
        min_swap_interval_slots: 25,
    }
}

/// Mint extensions used by tests.
pub enum Ext {
    None,
    Hook(Option<Pubkey>),
    TransferFee,
    NonTransferable,
    DefaultFrozen,
    PermanentDelegate,
}

pub struct Game {
    pub svm: LiteSVM,
    /// `$GRID`: Token-2022, mint authority ADMIN.
    pub token_mint: Pubkey,
}

impl Game {
    /// Loads the program and runs Initialize with `default_params()` (paused).
    pub fn new() -> Self {
        let mut game = Self::uninitialized();
        game.init().unwrap();
        game
    }

    /// Programs, funded keys, wSOL and `$GRID` mints; no Initialize yet.
    pub fn uninitialized() -> Self {
        let mut svm = LiteSVM::new().with_sigverify(false).with_transaction_history(0);
        svm.add_program_from_file(grid_api::ID, concat!(env!("CARGO_MANIFEST_DIR"), "/../target/deploy/grid.so"))
            .expect("run `cargo build-sbf` first");
        svm.add_program_from_file(VRF_PROGRAM_ID, concat!(env!("CARGO_MANIFEST_DIR"), "/../target/deploy/mock_vrf.so"))
            .expect("run `cargo build-sbf` first");
        svm.add_program_from_file(JUPITER_PROGRAM_ID, concat!(env!("CARGO_MANIFEST_DIR"), "/../target/deploy/mock_swap.so"))
            .expect("run `cargo build-sbf` first");
        // Mainnet's Tokenkeg binary (p-token), not LiteSVM's bundled classic spl-token.
        svm.add_program_from_file(spl_token::ID, concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/spl_token_mainnet.so"))
            .unwrap();
        let queue = Account { lamports: SOL, data: vec![0; 512], owner: VRF_PROGRAM_ID, executable: false, rent_epoch: 0 };
        svm.set_account(VRF_QUEUE, queue).unwrap();
        for key in [ADMIN, COLLECTOR, TEAM, WORKER] {
            svm.airdrop(&key, 100 * SOL).unwrap();
        }
        // The native (wSOL) mint, which exists on mainnet.
        let mut data = vec![0; spl_token::state::Mint::LEN];
        let native = spl_token::state::Mint { decimals: 9, is_initialized: true, ..Default::default() };
        spl_token::state::Mint::pack(native, &mut data).unwrap();
        let lamports = svm.minimum_balance_for_rent_exemption(data.len());
        svm.set_account(SOL_MINT, Account { lamports, data, owner: spl_token::ID, executable: false, rent_epoch: 0 })
            .unwrap();
        let mut game = Game { svm, token_mint: Pubkey::default() };
        game.token_mint = game.create_mint(true, GRID_DECIMALS, Ext::None);
        game
    }

    pub fn init(&mut self) -> TxResult {
        let args = Initialize {
            worker: WORKER,
            admin_collector: COLLECTOR,
            team_collector: TEAM,
            token_mint: self.token_mint,
            token_program: spl_token_2022::ID,
            token_decimals: GRID_DECIMALS as u64,
            params: default_params(),
        };
        self.send(&[initialize(ADMIN, args)], ADMIN)
    }

    /// Initialize + 25 SPL tiles + unpause.
    pub fn ready() -> Self {
        let mut game = Self::new();
        game.set_tiles();
        game.send(&[set_pause(ADMIN, false)], ADMIN).unwrap();
        game
    }

    pub fn send(&mut self, ixs: &[Instruction], payer: Pubkey) -> TxResult {
        let msg = Message::new_with_blockhash(ixs, Some(&payer), &self.svm.latest_blockhash());
        self.svm.send_transaction(Transaction::new_unsigned(msg)).map(|_| ()).map_err(|e| {
            e.meta.logs.iter().for_each(|l| eprintln!("{l}"));
            Box::new(e)
        })
    }

    /// A funded wallet.
    pub fn user(&mut self) -> Pubkey {
        let key = Pubkey::new_unique();
        self.svm.airdrop(&key, 100 * SOL).unwrap();
        key
    }

    pub fn create_mint(&mut self, t22: bool, decimals: u8, ext: Ext) -> Pubkey {
        let mint = Pubkey::new_unique();
        let program = if t22 { spl_token_2022::ID } else { spl_token::ID };
        let (types, init_ext) = match ext {
            Ext::None => (vec![], None),
            Ext::Hook(hook) => (
                vec![ExtensionType::TransferHook],
                Some(spl_token_2022::extension::transfer_hook::instruction::initialize(&program, &mint, Some(ADMIN), hook).unwrap()),
            ),
            Ext::TransferFee => (
                vec![ExtensionType::TransferFeeConfig],
                Some(spl_token_2022::extension::transfer_fee::instruction::initialize_transfer_fee_config(&program, &mint, Some(&ADMIN), Some(&ADMIN), 0, 0).unwrap()),
            ),
            Ext::NonTransferable => (
                vec![ExtensionType::NonTransferable],
                Some(spl_token_2022::instruction::initialize_non_transferable_mint(&program, &mint).unwrap()),
            ),
            Ext::DefaultFrozen => (
                vec![ExtensionType::DefaultAccountState],
                Some(spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(&program, &mint, &spl_token_2022::state::AccountState::Frozen).unwrap()),
            ),
            Ext::PermanentDelegate => (
                vec![ExtensionType::PermanentDelegate],
                Some(spl_token_2022::instruction::initialize_permanent_delegate(&program, &mint, &ADMIN).unwrap()),
            ),
        };
        let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&types).unwrap();
        let lamports = self.svm.minimum_balance_for_rent_exemption(space);
        let mut ixs = vec![system_instruction::create_account(&ADMIN, &mint, lamports, space as u64, &program)];
        ixs.extend(init_ext);
        ixs.push(spl_token_2022::instruction::initialize_mint2(&program, &mint, &ADMIN, Some(&ADMIN), decimals).unwrap());
        self.send(&ixs, ADMIN).unwrap();
        mint
    }

    /// Mints `$GRID` to `owner`'s ATA (created if needed).
    pub fn mint_grid(&mut self, owner: Pubkey, amount: u64) {
        let p = spl_token_2022::ID;
        let ixs = [
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(&ADMIN, &owner, &self.token_mint, &p),
            spl_token_2022::instruction::mint_to(&p, &self.token_mint, &ata(&owner, &self.token_mint, &p), &ADMIN, &[], amount).unwrap(),
        ];
        self.send(&ixs, ADMIN).unwrap();
    }

    pub fn grid_balance(&self, owner: Pubkey) -> u64 {
        self.svm
            .get_account(&ata(&owner, &self.token_mint, &spl_token_2022::ID))
            .map_or(0, |a| u64::from_le_bytes(a.data[64..72].try_into().unwrap()))
    }

    /// Sets all 25 tiles to fresh SPL mints.
    pub fn set_tiles(&mut self) {
        for i in 0..TILES as u64 {
            let mint = self.create_mint(false, 6, Ext::None);
            self.send(&[set_tile(ADMIN, i, mint)], ADMIN).unwrap();
        }
    }

    /// Mints `amount` `$GRID` to a funder and deposits it with FundReserve.
    pub fn fund_reserve(&mut self, amount: u64) {
        let funder = self.user();
        self.mint_grid(funder, amount);
        self.send(&[fund_reserve(funder, self.token_mint, spl_token_2022::ID, amount)], funder).unwrap();
    }

    pub fn deploy(&mut self, miner: Pubkey, amount: u64, mask: u64) -> TxResult {
        let round_id = self.board().round_id;
        self.send(&[deploy(miner, miner, amount, round_id, mask)], miner)
    }

    pub fn request_rng(&mut self, payer: Pubkey) -> TxResult {
        let round_id = self.board().round_id;
        self.send(&[request_rng(payer, round_id)], payer)
    }

    /// The mock VRF answers the last request with `value` (callback signed by the VRF identity).
    pub fn vrf_answer(&mut self, round_id: u64, value: [u8; 32]) -> TxResult {
        let mut data = vec![4];
        data.extend_from_slice(&value);
        let ix = Instruction {
            program_id: VRF_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(vrf_identity().0, false),
                AccountMeta::new(VRF_QUEUE, false),
                AccountMeta::new_readonly(grid_api::ID, false),
                AccountMeta::new(round_pda(round_id).0, false),
            ],
            data,
        };
        self.send(&[ix], ADMIN)
    }

    /// At end_slot: RequestRng, then the mock VRF answers with a value that draws `tile`.
    pub fn vrf_tile(&mut self, round_id: u64, tile: u64) {
        self.warp_to(self.board().end_slot);
        self.request_rng(ADMIN).unwrap();
        self.vrf_answer(round_id, rng_for_tile(round_id, tile)).unwrap();
    }

    /// Warps past the intermission and resets the current round.
    pub fn reset(&mut self) -> TxResult {
        let board = self.board();
        self.warp_to(board.end_slot + default_params().intermission_slots);
        let res = self.send(&[reset(ADMIN, board.round_id)], ADMIN);
        let slot = self.svm.get_sysvar::<solana_sdk::clock::Clock>().slot;
        self.warp_to(slot + 1); // next round opens at slot + 1
        res
    }

    /// Deploys, gets a VRF value that draws `winning_tile` and resets: one full round.
    pub fn play_round(&mut self, deploys: &[(Pubkey, u64, u64)], winning_tile: u64) -> u64 {
        let round_id = self.board().round_id;
        for &(miner, amount, mask) in deploys {
            self.deploy(miner, amount, mask).unwrap();
        }
        self.vrf_tile(round_id, winning_tile);
        self.reset().unwrap();
        round_id
    }

    pub fn checkpoint(&mut self, miner: Pubkey, round_id: u64) -> TxResult {
        self.send(&[checkpoint(miner, miner, round_id)], miner)
    }

    /// Creates `owner`'s ATA for `mint` if needed and mints `amount` to it (test mints: ADMIN authority).
    pub fn mint_to(&mut self, owner: Pubkey, mint: Pubkey, program: Pubkey, amount: u64) {
        let mut ixs = vec![spl_associated_token_account::instruction::create_associated_token_account_idempotent(&ADMIN, &owner, &mint, &program)];
        if amount > 0 {
            ixs.push(spl_token_2022::instruction::mint_to(&program, &mint, &ata(&owner, &mint, &program), &ADMIN, &[], amount).unwrap());
        }
        self.send(&ixs, ADMIN).unwrap();
    }

    pub fn token_balance(&self, owner: Pubkey, mint: Pubkey, program: Pubkey) -> u64 {
        self.svm
            .get_account(&ata(&owner, &mint, &program))
            .filter(|a| a.data.len() >= 72)
            .map_or(0, |a| u64::from_le_bytes(a.data[64..72].try_into().unwrap()))
    }

    /// A mock Jupiter route (see `mock_swap`): takes `amount_in` wSOL from the swap PDA's ATA and
    /// pays `amount_out` of `mint` into the Treasury's ATA (`mode` 0 honest, 1 short, 2 steals).
    pub fn route(&mut self, mode: u8, amount_in: u64, amount_out: u64, mint: Pubkey, program: Pubkey, extra: &[Pubkey]) -> Instruction {
        let src = Pubkey::find_program_address(&[b"src"], &JUPITER_PROGRAM_ID).0;
        let (swap, treasury) = (swap_pda().0, treasury_pda().0);
        self.mint_to(src, SOL_MINT, spl_token::ID, 0);
        self.mint_to(src, mint, program, amount_out);
        self.mint_to(treasury, mint, program, 0);
        let mut accounts = vec![
            AccountMeta::new_readonly(swap, false),
            AccountMeta::new(ata(&swap, &SOL_MINT, &spl_token::ID), false),
            AccountMeta::new(ata(&src, &SOL_MINT, &spl_token::ID), false),
            AccountMeta::new(ata(&src, &mint, &program), false),
            AccountMeta::new(ata(&treasury, &mint, &program), false),
            AccountMeta::new_readonly(src, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(program, false),
        ];
        accounts.extend(extra.iter().map(|k| AccountMeta::new(*k, false)));
        let mut data = vec![mode];
        data.extend_from_slice(&amount_in.to_le_bytes());
        data.extend_from_slice(&amount_out.to_le_bytes());
        Instruction { program_id: JUPITER_PROGRAM_ID, accounts, data }
    }

    /// Worker BuyAsset through a mock route with `min_out = amount_out`, after the swap interval.
    pub fn buy(&mut self, kind: u64, tile: u64, amount_in: u64, amount_out: u64, mode: u8) -> TxResult {
        let t = self.config().tiles[tile as usize];
        let program = t.token_program();
        let route = self.route(mode, amount_in, amount_out, t.mint, program, &[]);
        self.after_swap_interval();
        self.send(&[buy_asset(WORKER, kind, tile, amount_in, amount_out, t.mint, program, route)], WORKER)
    }

    pub fn after_swap_interval(&mut self) {
        let next = self.config().last_swap_slot + default_params().min_swap_interval_slots;
        let slot = self.svm.get_sysvar::<solana_sdk::clock::Clock>().slot;
        self.warp_to(slot.max(next));
    }

    /// Tile `i`'s Treasury ATA holds exactly its books: pot + vault + owed by open rounds.
    pub fn assert_tile_books(&self, i: usize) {
        let t = self.treasury();
        let tile = self.config().tiles[i];
        let held = self.token_balance(treasury_pda().0, tile.mint, tile.token_program());
        assert_eq!(held, t.pot_tokens[i] + t.vault[i] + t.owed[i], "tile {i} books");
    }

    pub fn set_params(&mut self, params: Params) -> TxResult {
        let c = self.config();
        let args = UpdateConfig {
            admin: c.admin,
            worker: c.worker,
            admin_collector: c.admin_collector,
            team_collector: c.team_collector,
            params,
        };
        self.send(&[update_config(ADMIN, args)], ADMIN)
    }
    pub fn warp_to(&mut self, slot: u64) {
        self.svm.warp_to_slot(slot);
        self.svm.expire_blockhash();
    }

    pub fn read<T: Pod>(&self, address: Pubkey) -> T {
        let data = self.svm.get_account(&address).expect("account exists").data;
        bytemuck::pod_read_unaligned(&data[8..8 + core::mem::size_of::<T>()])
    }

    /// Edits a program account in place (test setup only).
    pub fn update<T: Pod>(&mut self, address: Pubkey, f: impl FnOnce(&mut T)) {
        let mut account: Account = self.svm.get_account(&address).unwrap();
        let mut value = self.read::<T>(address);
        f(&mut value);
        account.data[8..8 + core::mem::size_of::<T>()].copy_from_slice(bytemuck::bytes_of(&value));
        self.svm.set_account(address, account).unwrap();
    }

    pub fn lamports(&self, address: Pubkey) -> u64 {
        self.svm.get_balance(&address).unwrap_or(0)
    }

    pub fn board(&self) -> Board {
        self.read(board_pda().0)
    }
    pub fn config(&self) -> Config {
        self.read(config_pda().0)
    }
    pub fn treasury(&self) -> Treasury {
        self.read(treasury_pda().0)
    }
    pub fn round(&self, id: u64) -> Round {
        self.read(round_pda(id).0)
    }
    pub fn miner(&self, authority: Pubkey) -> Miner {
        self.read(miner_pda(authority).0)
    }
}

/// The custom program error code of a failed transaction.
pub fn error_code(res: TxResult) -> u32 {
    match res.expect_err("expected failure").err {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => code,
        other => panic!("not a custom error: {other:?}"),
    }
}

/// A VRF value that makes `round_id` draw `tile`.
pub fn rng_for_tile(round_id: u64, tile: u64) -> [u8; 32] {
    (1u64..)
        .map(|i| {
            let mut v = [1u8; 32];
            v[..8].copy_from_slice(&i.to_le_bytes());
            v
        })
        .find(|v| draw(v, round_id, b"tile", TILES as u64) == tile)
        .unwrap()
}

/// SOL (lamports) and `$GRID` the Treasury must hold per its books: rent + SOL buckets, and
/// reserve + Vault `$GRID` + refined/unrefined + `$GRID` that unclosed rounds still owe.
pub fn treasury_books(game: &Game, open_rounds: &[u64]) -> (u64, u64) {
    let t = game.treasury();
    let rent = game.svm.minimum_balance_for_rent_exemption(8 + core::mem::size_of::<Treasury>());
    let sol = rent + t.sol_books().unwrap();
    let in_flight: u64 = open_rounds
        .iter()
        .map(|&id| game.round(id))
        .map(|r| r.token_reward - r.token_credited + r.vault[VAULT_GRID] - r.vault_paid[VAULT_GRID])
        .sum();
    (sol, t.reserve + t.vault[VAULT_GRID] + t.total_refined + t.total_unrefined + in_flight)
}

/// A mask with only `tile` set.
pub fn tile(tile: u64) -> u64 {
    1 << tile
}
