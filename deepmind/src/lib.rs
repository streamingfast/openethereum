use std::{fmt, sync::Arc};
use rustc_hex::ToHex;
use ethereum_types as eth;

static EMPTY_BYTES: [u8; 0] = [];
#[derive(Debug, PartialEq, Clone)]
pub struct Config {
    pub enabled: bool,
    pub on_chain_sync: bool,
    pub on_block_progress: bool,
}

impl Config {
	pub fn disabled() -> Self {
		Default::default()
	}
}

impl Default for Config {
	fn default() -> Self {
		Config {
			enabled: false,
			on_chain_sync: false,
			on_block_progress: false,
		}
	}
}

pub trait Printer: Send + Sync {
    fn print(&self, _input: &str) {}
}

pub struct DiscardPrinter {
}

impl Printer for DiscardPrinter {
}

pub struct IoPrinter {
    // io: Box<dyn Write + Send + Sync>
}

impl Printer for IoPrinter {
    fn print(&self, input: &str) {
        println!("DMLOG {}", input);
        // if let Err(err) = self.io.write_all(b"DMLOG ") {
        //     panic!("Unable to full write line to I/O {}", err);
        // }

        // if let Err(err) = self.io.write_all(input.as_bytes()) {
        //     panic!("Unable to full write line to I/O {}", err);
        // }

        // if let Err(err) = self.io.write_all(b"\n") {
        //     panic!("Unable to full write line to I/O {}", err);
        // }
    }
}

pub trait Tracer: Send {
    fn is_enabled(&self) -> bool { false }
    fn start_call(&mut self, _call: Call) {}
    fn reverted_call(&self, _gas_left: &eth::U256) {}
    fn failed_call(&mut self, _gas_left: &eth::U256, _gas_left_after_failure: &eth::U256, _err: &String) {}
    fn end_call(&mut self, _gas_left: &eth::U256, _return_data: &[u8]) {}
    fn end_failed_call(&mut self) {}

    // Those are done integrating into OpenEthereum code, and they count matched Geth version (still missing 1 - 1 comparison)
    fn record_balance_change(&mut self, _address: &eth::Address, _old: &eth::U256, _new: &eth::U256, _reason: BalanceChangeReason) {}
    fn record_nonce_change(&mut self, _address: &eth::Address, _old: &eth::U256, _new: &eth::U256) {}
    fn record_keccak(&mut self, _hash_of_data: &eth::H256, _data: &[u8]) {}

    // Those are NOT integrated yet into OpenEthereum, they should work once printed, needs to validate that count match with Geth prior moving it above
    fn record_call_without_code(&mut self) {}
    fn record_gas_refund(&mut self, _gas_old: u64, _gas_refund: u64) {}
    fn record_gas_consume(&mut self, _gas_old: u64, _gas_consumed: u64, _reason: GasChangeReason) {}
    fn record_storage_change(&mut self, _addr: &eth::Address, _key: &eth::H256, _old_data: &eth::H256, _new_data: &eth::H256) {}
    fn record_suicide(&mut self, _addr: &eth::Address, _suicided: bool, _balance_before_suicide: &eth::U256) {}
    fn record_new_account(&mut self, _addr: &eth::Address) {}
    fn record_code_change(&mut self, _addr: &eth::Address, _input_hash: &eth::H256, _code_hash: &eth::H256, _old_code: &[u8], _new_code: &[u8]) {}
    fn record_before_call_gas_event(&mut self, _gas_value: u64) {}
    fn record_after_call_gas_event(&mut self, _gas_value: u64) {}

    // fn record_log(&mut self, log: *types.Log) {}
    // fn record_trx_pool(&mut self, event_type: string, tx *types.Transaction, err error) {}

    fn debug(&mut self, _input: String) {}
}

pub struct NoopTracer;

impl Tracer for NoopTracer {
}

/// BlockTracer is responsible of recording single tracing elements (like Balance or Gas change)
/// that happens outside of any transactions on a block.
pub struct BlockTracer {
    printer: Arc<Box<dyn Printer>>,
}

impl Tracer for BlockTracer {
    fn is_enabled(&self) -> bool {
        return true;
    }

    fn record_balance_change(&mut self,
        address: &eth::Address,
        old: &eth::U256,
        new: &eth::U256,
        reason: BalanceChangeReason,
    ) {
        record_balance_change(&self.printer, 0, address, old, new, reason)
    }
}

/// TransactionTracer is responsible of transaction tracing level with mutability like the ability to track the
/// actual call index we are currently at. Aside mutability and state, it delegates all Deep Mind
/// printing operations to the `Context`.
pub struct TransactionTracer {
    printer: Arc<Box<dyn Printer>>,
    call_index: u64,
    call_stack: Vec<u64>,
    gas_left_after_latest_failure: Option<eth::U256>,
}

impl Tracer for TransactionTracer {
    fn is_enabled(&self) -> bool {
        return true;
    }

    fn start_call(&mut self, call: Call) {
        self.call_index += 1;
        self.call_stack.push(self.call_index);

        self.printer.print(format!("EVM_RUN_CALL {call_type} {call_index}",
            call_type = call.call_type,
            call_index = self.call_index,
        ).as_ref());

        self.printer.print(format!("EVM_PARAM {call_type} {call_index} {from:x} {to:x} {value:x} {gas_limit} {input:x}",
            call_type = call.call_type,
            call_index = self.call_index,
            from = Address(&call.from),
            to = Address(&call.to),
            value = U256(&call.value.unwrap_or_else(|| eth::U256::from(0))),
            gas_limit = call.gas_limit,
            input = Hex(call.input.unwrap_or(&EMPTY_BYTES)),
        ).as_ref());
    }

    fn reverted_call(&self, gas_left: &eth::U256) {
        self.printer.print(format!("EVM_CALL_FAILED {call_index} {gas_left} {reason}",
            call_index = self.active_call_index(),
            gas_left = gas_left.as_u64(),
            reason = "Reverted",
        ).as_ref());

        self.printer.print(format!("EVM_REVERTED {call_index}",
            call_index = self.active_call_index(),
        ).as_ref());
    }

    fn failed_call(&mut self, gas_left: &eth::U256, gas_left_after_failure: &eth::U256, err: &String) {
        if self.gas_left_after_latest_failure.is_some() {
            panic!("There is already a gas_left_after_latest_failure value set at this point that should have been consumed already")
        }

        self.printer.print(format!("EVM_CALL_FAILED {call_index} {gas_left} {reason}",
            call_index = self.active_call_index(),
            gas_left = gas_left.as_u64(),
            reason = err,
        ).as_ref());

        self.gas_left_after_latest_failure = Some(*gas_left_after_failure);
    }

    fn end_call(&mut self, gas_left: &eth::U256, return_data: &[u8]) {
       let call_index = match self.call_stack.pop() {
           Some(index) => index,
           None => panic!("There should always be a call in our call index stack")
       };

        self.printer.print(format!("EVM_END_CALL {call_index} {gas_left:} {return_value:x}",
            call_index = call_index,
            gas_left = gas_left.as_u64(),
            return_value = Hex(return_data),
        ).as_ref());
    }

    fn end_failed_call(&mut self) {
        let gas_left = match self.gas_left_after_latest_failure {
            Some(amount) => amount,
            None => panic!("There should be a gas_left_after_latest_failure value set at this point")
        };

        self.gas_left_after_latest_failure = None;

        self.end_call(&gas_left, &[])
    }

    fn record_balance_change(&mut self, address: &eth::Address, old: &eth::U256, new: &eth::U256, reason: BalanceChangeReason) {
        record_balance_change(&self.printer, self.active_call_index() as u64, address, old, new, reason)
    }

    fn record_nonce_change(&mut self, address: &eth::Address, old: &eth::U256, new: &eth::U256) {
        self.printer.print(format!("NONCE_CHANGE {call_index} {address:x} {old_nonce} {new_nonce}",
            call_index = self.active_call_index(),
            address = Address(address),
            old_nonce = old.as_u64(),
            new_nonce = new.as_u64(),
        ).as_ref());
    }

    fn record_keccak(&mut self, hash_of_data: &eth::H256, data: &[u8]) {
        self.printer.print(format!("EVM_KECCAK {call_index} {hash_of_data:x} {data:x}",
            call_index = self.active_call_index(),
            hash_of_data = H256(hash_of_data),
            data = Hex(data),
        ).as_ref());
    }

    fn record_call_without_code(&mut self) {
        self.printer.print(format!("ACCOUNT_WITHOUT_CODE {call_index}",
            call_index = self.active_call_index(),
        ).as_ref());
    }

    fn record_gas_consume(&mut self, gas_old: u64, gas_consumed: u64, reason: GasChangeReason) {
        if gas_consumed != 0 {
            record_gas_change(&self.printer, self.active_call_index(), gas_old, gas_consumed-gas_consumed, reason);
        }
    }

    fn record_gas_refund(&mut self, gas_old: u64, gas_refund: u64) {
        if gas_refund != 0 {
            record_gas_change(&self.printer, self.active_call_index(), gas_old, gas_old+gas_refund, GasChangeReason::RefundAfterExecution);
        }
    }

    fn record_storage_change(&mut self, address: &eth::Address, key: &eth::H256, old_data: &eth::H256, new_data: &eth::H256) {
        self.printer.print(format!("STORAGE_CHANGE {call_index} {address:x} {key:x} {old_data:x} {new_data:x}",
            call_index = self.active_call_index(),
            address = Address(address),
            key = H256(key),
            old_data = H256(old_data),
            new_data = H256(new_data),
        ).as_ref());
    }

    fn record_suicide(&mut self, address: &eth::Address, suicided: bool, balance_before_suicide: &eth::U256) {
        self.printer.print(format!("SUICIDE_CHANGE {call_index} {address:x} {suicided} {balance_before_suicide:x}",
            call_index = self.active_call_index(),
            address = Address(address),
            suicided = suicided,
            balance_before_suicide = U256(balance_before_suicide),
        ).as_ref());

        // Matt: I don't think this is required by OpenEthereum since handle differently, needs to be validated
        // if balanceBeforeSuicide.Sign() != 0 {
        // 	// We need to explicit add a balance change removing the suicided contract balance since
        // 	// the remaining balance of the contract has already been resetted to 0 by the time we
        // 	// do the print call.
        // 	ctx.RecordBalanceChange(addr, balanceBeforeSuicide, common.Big0, BalanceChangeReason("suicide_withdraw"))
        // }
    }

    fn record_new_account(&mut self, address: &eth::Address) {
        self.printer.print(format!("CREATED_ACCOUNT {call_index} {address:x}",
            call_index = self.active_call_index(),
            address = Address(address),
        ).as_ref());
    }

    fn record_code_change(&mut self, address: &eth::Address, input_hash: &eth::H256, code_hash: &eth::H256, old_code: &[u8], new_code: &[u8]) {
        self.printer.print(format!("CODE_CHANGE {call_index} {address:x} {input_hash:x} {old_code:x} {code_hash:x} {new_code:x}",
            // Follows Geth order, yes it's not aligned with the record_code_change signature, but we must respect console reader order here
            call_index = self.active_call_index(),
            address = Address(address),
            input_hash = H256(input_hash),
            old_code = Hex(old_code),
            code_hash = H256(code_hash),
            new_code = Hex(new_code),
        ).as_ref());
    }

    fn record_before_call_gas_event(&mut self, gas_value: u64) {
        // // The `ctx.nextCallIndex` has not been incremented yet, so we add +1 for the linked call index
        // ctx.printer.Print("GAS_EVENT", ctx.callIndex(), Uint64(ctx.nextCallIndex+1), "before_call", Uint64(gasValue))
        let call_index = self.active_call_index();

        // Matt: Validate against Geth logic, see commented code at top of this implementation
        self.printer.print(format!("GAS_EVENT {call_index} {for_call_index} {reason} {gas_value}",
            call_index = call_index,
            for_call_index = call_index + 1,
            reason = "before_call",
            gas_value = gas_value,
        ).as_ref());
    }

    fn record_after_call_gas_event(&mut self, gas_value: u64) {
        // // The `ctx.nextCallIndex` is already pointing to previous call index, so we simply use it for the linked call index
	    // ctx.printer.Print("GAS_EVENT", ctx.callIndex(), Uint64(ctx.nextCallIndex), "after_call", Uint64(gasValue))
        let call_index = self.active_call_index();

        // Matt: Validate against Geth logic, see commented code at top of this implementation
        self.printer.print(format!("GAS_EVENT {call_index} {for_call_index} {reason} {gas_value}",
            call_index = call_index,
            for_call_index = call_index,
            reason = "before_call",
            gas_value = gas_value,
        ).as_ref());
    }

    fn debug(&mut self, input: String) {
        self.printer.print(&input);
    }
}

impl TransactionTracer {
    fn active_call_index(&self) -> u64 {
        if self.call_stack.len() <= 0 {
            // There is some balance change(s) in a transaction that happens before any call has been scheduled yet,
            // this is the case for the initial gas buying for example. If the call stack is empty, let's return
            // active call index 0 and the console reader process deals with it and attach it to the root call of
            // the transaction.
            return 0
        }

        self.call_stack[self.call_stack.len() - 1]
    }
}

#[inline]
fn record_balance_change(
    printer: &Box<dyn Printer>,
    call_index: u64,
    address: &eth::Address,
    old: &eth::U256,
    new: &eth::U256,
    reason: BalanceChangeReason,
) {
    if reason != BalanceChangeReason::Ignored {
        printer.print(format!("BALANCE_CHANGE {call_index} {address:x} {old_balance:x} {new_balance:x} {reason}",
            call_index = call_index,
            address = Address(address),
            old_balance = U256(old),
            new_balance = U256(new),
            reason = reason,
        ).as_ref())
    }
}

#[inline]
fn record_gas_change(
    printer: &Box<dyn Printer>,
    call_index: u64,
    old: u64,
    new: u64,
    reason: GasChangeReason,
) {
    if reason != GasChangeReason::Ignored {
        printer.print(format!("GAS_CHANGE {call_index} {old_gas} {new_gas} {reason}",
            call_index = call_index,
            old_gas = old,
            new_gas = new,
            reason = reason,
        ).as_ref())
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BalanceChangeReason {
    Unknown,
    RewardMineUncle,
    RewardMineBlock,
    DaoRefundContract,
    DaoAdjustBalance,
    Transfer,
    GenesisBalance,
    GasBuy,
    RewardTransactionFee,
    GasRefund,
    TouchAccount,
    SuicideRefund,
    SuicideWithdraw,
    CallBalanceOverride,

    // Special enum that should be ignored when writing, should never be displayed
    Ignored,
}

impl fmt::Display for BalanceChangeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The output must match exact names found in `proto-ethereum/dfuse/ethereum/codec/v1/codec.proto#BalanceChange.Reason` enum, which is also respected by Geth
        f.write_str(match self {
            BalanceChangeReason::Unknown => "unknown",
            BalanceChangeReason::RewardMineUncle => "reward_mine_uncle",
            BalanceChangeReason::RewardMineBlock => "reward_mine_block",
            BalanceChangeReason::DaoRefundContract => "dao_refund_contract",
            BalanceChangeReason::DaoAdjustBalance => "dao_adjust_balance",
            BalanceChangeReason::Transfer => "transfer",
            BalanceChangeReason::GenesisBalance => "genesis_balance",
            BalanceChangeReason::GasBuy => "gas_buy",
            BalanceChangeReason::RewardTransactionFee => "reward_transaction_fee",
            BalanceChangeReason::GasRefund => "gas_refund",
            BalanceChangeReason::TouchAccount => "touch_account",
            BalanceChangeReason::SuicideRefund => "suicide_refund",
            BalanceChangeReason::SuicideWithdraw => "suicide_withdraw",

            // Those that should actually results in panics
            BalanceChangeReason::CallBalanceOverride => panic!("A CallBalanceOverride balance change reason should never be used"),
            BalanceChangeReason::Ignored => panic!("A Ignored balance change reason should never be displayed")
        })
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum GasChangeReason {
    Unknown,
    Call,
    CallCode,
    CallDataCopy,
    CodeCopy,
    CodeStorage,
    ContractCreation,
    ContractCreation2,
    DelegateCall,
    EventLog,
    ExtCodeCopy,
    FailedExecution,
    IntrinsicGas,
    PrecompiledContract,
    RefundAfterExecution,
    Return,
    ReturnDataCopy,
    Revert,
    SelfDestruct,
    StaticCall,

    // Added in Berlin fork (Geth 1.10+)
    StateColdAccess,

    // Special enum that should be ignored when writing, should never be displayed
    Ignored,
}

impl fmt::Display for GasChangeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The output must match exact names found in `proto-ethereum/dfuse/ethereum/codec/v1/codec.proto#GasChange.Reason` enum, which is also respected by Geth
        f.write_str(match self {
            GasChangeReason::Unknown => "unknown",
            GasChangeReason::Call => "call",
            GasChangeReason::CallCode => "call_code",
            GasChangeReason::CallDataCopy => "call_data_copy",
            GasChangeReason::CodeCopy => "code_copy",
            GasChangeReason::CodeStorage => "code_storage",
            GasChangeReason::ContractCreation => "contract_creation",
            GasChangeReason::ContractCreation2 => "contract_creation2",
            GasChangeReason::DelegateCall => "delegate_call",
            GasChangeReason::EventLog => "event_log",
            GasChangeReason::ExtCodeCopy => "ext_code_copy",
            GasChangeReason::FailedExecution => "failed_execution",
            GasChangeReason::IntrinsicGas => "intrinsic_gas",
            GasChangeReason::PrecompiledContract => "precompiled_contract",
            GasChangeReason::RefundAfterExecution => "refund_after_execution",
            GasChangeReason::Return => "return",
            GasChangeReason::ReturnDataCopy => "return_data_copy",
            GasChangeReason::Revert => "revert",
            GasChangeReason::SelfDestruct => "self_destruct",
            GasChangeReason::StaticCall => "static_call",
            GasChangeReason::StateColdAccess => "state_cold_access",

            GasChangeReason::Ignored => panic!("A Ignored gas change reason should never be displayed")
        })
    }
}


#[derive(Debug, PartialEq, Clone)]
pub enum Instrumentation {
    Full,
    BlockProgress,
    None,
}

pub struct Context {
    instrumentation: Instrumentation,
    printer: Arc<Box<dyn Printer>>,
}

impl Context {
    pub fn new(instrumentation: Instrumentation) -> Context {
        Context {
            instrumentation,
            // printer: Box::new(IoPrinter{io: Box::new(std::io::stdout())}),
            printer: Arc::new(Box::new(IoPrinter{})),
        }
    }

    pub fn noop() -> Context {
        Context {
            instrumentation: Instrumentation::None,
            printer: Arc::new(Box::new(DiscardPrinter{})),
        }
    }

    pub fn block_tracer(&self) -> BlockTracer {
        BlockTracer{printer: self.printer.clone()}
    }

    pub fn tracer(&self) -> TransactionTracer {
        TransactionTracer{printer: self.printer.clone(), call_index: 0, call_stack: Vec::with_capacity(16), gas_left_after_latest_failure: None}
    }

    pub fn is_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full;
    }

    pub fn is_finalize_block_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full || self.instrumentation == Instrumentation::BlockProgress;
    }

    pub fn start_block(&self, num: u64) {
        self.printer.print(format!("BEGIN_BLOCK {num}", num = num).as_ref())
    }

    pub fn finalize_block(&self, num: u64) {
        self.printer.print(format!("FINALIZE_BLOCK {num}", num = num).as_ref())
    }

    pub fn end_block(&self, num: u64, size: u64, /*, header, uncle_headers */) {
        self.printer.print(format!("END_BLOCK {num} {size}",
            num = num,
            size = size,
        ).as_ref())
    }

    pub fn start_transaction(&self, trx: Transaction) {
        let (v, r, s) = trx.signature;
        let mut to_str = ".".to_owned();
        if let Some(ref address) = trx.to {
            to_str = format!("{:x}", Address(address));
        }

        self.printer.print(format!("BEGIN_APPLY_TRX {hash:x} {to} {value:x} {v:x} {r:x} {s:x} {gas_limit} {gas_price:x} {nonce} {data:x}",
            hash = H256(&trx.hash),
            to = to_str,
            value = U256(&trx.value),
            v = v,
            r = H256(&r),
            s = H256(&s),
            gas_limit = &trx.gas_limit,
            gas_price = U256(&trx.gas_price),
            nonce = &trx.nonce,
            data = Hex(&trx.data),
        ).as_ref());

        self.printer.print(format!("TRX_FROM {from:x}", from = Address(&trx.from)).as_ref());
    }

    pub fn end_transaction(&self) {
        self.printer.print(format!("END_APPLY_TRX").as_ref())
    }
}

struct Address<'a>(&'a eth::Address);

impl fmt::LowerHex for Address<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}

pub enum CallType {
    Call,
    CallCode,
    Create,
    Create2,
    DelegateCall,
    StaticCall,
}

impl fmt::Display for CallType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       return f.write_str(match self {
            CallType::Call => "CALL",
            CallType::CallCode => "CALLCODE",
            CallType::Create => "CREATE",
            CallType::Create2 => "CREATE",
            CallType::DelegateCall => "DELEGATE",
            CallType::StaticCall => "STATIC",
        })
    }
}

pub struct Call<'a> {
    pub call_type: CallType,
    pub from: eth::Address,
    pub to: eth::Address,
    pub value: Option<eth::U256>,
    pub gas_limit: u64,
    pub input: Option<&'a [u8]>,
}

pub struct Transaction<'a> {
    pub hash: eth::H256,
    pub from: eth::Address,
    pub to: Option<eth::Address>,
    pub value: eth::U256,
    pub gas_limit: u64,
    pub gas_price: eth::U256,
    pub nonce: u64,
    pub data: &'a [u8],
    pub signature: (u64, eth::H256, eth::H256),
}

struct Hex<'a>(&'a [u8]);

impl fmt::LowerHex for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() == 0 {
            return f.write_str(".")
        }

        f.write_str(self.0.to_hex::<String>().as_ref())
    }
}

struct H256<'a>(&'a eth::H256);

impl fmt::LowerHex for H256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}

struct U256<'a>(&'a eth::U256);

impl fmt::LowerHex for U256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}