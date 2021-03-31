use std::{fmt, sync::Arc};
use rustc_hex::ToHex;
use ethereum_types as eth;
use serde::{Serialize, Serializer};
use parity_version::{ to_deepmind_version };

pub static EMPTY_BYTES: [u8; 0] = [];
pub static PLATFORM: &str = "openethereum";
pub static FORK: &str = "vanilla";
pub static PROTOCOL_MAJOR: u64 = 1;
pub static PROTOCOL_MINOR: u64 = 0;


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

    fn debug(&self, _input: &str) {}
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

    /// Prints to the printer but not using DMLOG for now, this is to avoid
    /// console reader problems where it does not discard DMLOG messages that
    /// it don't understand.
    ///
    /// Remove this once the console reader has been fixed to simply discard
    /// messages that it doesn't know about.
    fn debug(&self, input: &str) {
        println!("DMDEBUG {}", input);
    }
}

pub trait Tracer: Send {
    fn is_enabled(&self) -> bool { false }

    fn start_call(&mut self, _call: Call) {}
    fn reverted_call(&self, _gas_left: &eth::U256) {}
    fn failed_call(&mut self, _gas_left_after_failure: &eth::U256, _err: String) {}
    fn end_call(&mut self, _gas_left: &eth::U256, _return_data: Option<&[u8]>) {}
    fn seen_failed_call(&mut self) -> bool { return false }
    fn end_failed_call(&mut self, _from: &str) {}

    fn record_balance_change(&mut self, _address: &eth::Address, _old: &eth::U256, _new: &eth::U256, _reason: BalanceChangeReason) {}
    fn record_nonce_change(&mut self, _address: &eth::Address, _old: &eth::U256, _new: &eth::U256) {}
    fn record_keccak(&mut self, _hash_of_data: &eth::H256, _data: &[u8]) {}
    fn record_new_account(&mut self, _addr: &eth::Address) {}
    fn record_suicide(&mut self, _addr: &eth::Address, _already_suicided: bool, _balance_before_suicide: &eth::U256) {}
    fn record_storage_change(&mut self, _addr: &eth::Address, _key: &eth::H256, _old_data: &eth::H256, _new_data: &eth::H256) {}
    fn record_log(&mut self, _log: Log) {}
    fn record_call_without_code(&mut self) {}

    fn record_gas_refund(&mut self, _gas_old: usize, _gas_refund: usize) {}
    fn record_gas_consume(&mut self, _gas_old: usize, _gas_consumed: usize, _reason: GasChangeReason) {}
    fn record_code_change(&mut self, _addr: &eth::Address, _input_hash: &eth::H256, _code_hash: &eth::H256, _old_code: &[u8], _new_code: &[u8]) {}
    fn record_before_call_gas_event(&mut self, _gas_value: usize) {}
    fn record_after_call_gas_event(&mut self, _gas_value: usize) {}

    /// Returns the number of Ethereum Log that was performed as part of this tracer
    fn get_log_count(&self) -> u64 { return 0 }

    /// Use this to add printing statement useful for debugging, the message is printed with the current
    /// tracer context like active call index and other tracer state information.
    fn debug(&mut self, _message: String) {}
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
	hash: eth::H256,
    printer: Arc<Box<dyn Printer>>,
    call_index: u64,
	last_pop_call_index: Option<u64>,
    call_stack: Vec<u64>,
	gas_event_call_stack: Vec<u64>,
    active_gas_left_at_failure: Option<eth::U256>,
    log_in_block_index: u64,
    log_count: u64,
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


	// gas_left: This is the gas that was left at the point of failure. This value will be depleted once the call has ended
	// i.e.
	// 	EVM_RUN_CALL 1 2,000 					// you have 2,000 gas left
	// 	...
	// 	EVM_CALL_FAILED 1 1300 Invalid 			// the call used up 700 gas and failed, thus you have left 1300 = 2000 - 7000
	// 	GAS_CHANGE 1300 0 EVM::Call:Failed 		// once the call is completed we depleted the remaining gas
	// 	EVM_END_CALL 1
    fn failed_call(&mut self, gas_left_at_failure: &eth::U256, err: String) {
        if self.active_gas_left_at_failure.is_some() {
            panic!("There is already a active_gas_left_at_failure value set at this point that should have been consumed already [{:?}], error is [{:?}]", self.hash, err)
        }

        self.printer.print(format!("EVM_CALL_FAILED {call_index} {gas_left} {reason}",
            call_index = self.active_call_index(),
            gas_left = gas_left_at_failure.as_u64(),
            reason = err,
        ).as_ref());

        self.active_gas_left_at_failure = Some(*gas_left_at_failure);
    }

    fn end_call(&mut self, gas_left: &eth::U256, return_data: Option<&[u8]>) {
        let call_index = match self.call_stack.pop() {
            Some(index) => index,
			None => panic!("There should always be a call in our call index stack [{:?}]",self.hash)
        };

        let mut return_bytes: &[u8] = &EMPTY_BYTES;
        if let Some(bytes) = return_data {
            return_bytes = bytes
        }

        self.printer.print(format!("EVM_END_CALL {call_index} {gas_left:} {return_value:x}",
            call_index = call_index,
            gas_left = gas_left.as_u64(),
            return_value = Hex(return_bytes),
        ).as_ref());

        self.last_pop_call_index = Some(call_index);
    }

    fn seen_failed_call(&mut self) -> bool {
        self.active_gas_left_at_failure.is_some()
    }

    fn end_failed_call(&mut self, from: &str) {
	    let gas_left_at_failure = match self.active_gas_left_at_failure {
            Some(amount) => amount,
            None => panic!("There should be a active_gas_left_at_failure value set at {} [{:?}]", from, self.hash)
        };
		self.active_gas_left_at_failure = None;

		// When a failed call occurs, the assumption is that the gas left after the latest
		// instruction failue is depleted to 0. Note, if the last instruction failed because of an OutOfGas error
		// we will simply deplete 0 to 0, maybe we should condition this not to happen?
		// Once the remaining has was consumed we push an end_call with 0 gas left
		self.record_gas_consume(gas_left_at_failure.as_usize(), gas_left_at_failure.as_usize(), GasChangeReason::FailedExecution);
        self.end_call(&eth::U256::from(0), None)
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

    fn record_gas_consume(&mut self, gas_old: usize, gas_consumed: usize, reason: GasChangeReason) {
        if gas_consumed != 0 {
            record_gas_change(&self.printer, self.active_call_index(), gas_old as u64, (gas_old as u64)-(gas_consumed as u64), reason);
        }
    }

    fn record_gas_refund(&mut self, gas_old: usize, gas_refund: usize) {
        if gas_refund != 0 {
            record_gas_change(&self.printer, self.active_call_index(), gas_old as u64, (gas_old as u64)+(gas_refund as u64), GasChangeReason::RefundAfterExecution);
        }
    }

    fn record_log(&mut self, log: Log) {
        let topics: Vec<String> = log.topics.iter().map(|topic| H256(topic).to_hex()).collect();

        self.printer.print(format!("ADD_LOG {call_index} {log_index_in_block} {address:x} {topics} {data:x}",
            call_index = self.active_call_index(),
            log_index_in_block = self.log_in_block_index,
            address = Address(&log.address),
            topics = topics.join(","),
            data = log.data,
        ).as_ref());

        self.log_count += 1;
        self.log_in_block_index += 1;
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

    fn record_suicide(&mut self, address: &eth::Address, already_suicided: bool, balance_before_suicide: &eth::U256) {
        self.printer.print(format!("SUICIDE_CHANGE {call_index} {address:x} {already_suicided} {balance_before_suicide:x}",
            call_index = self.active_call_index(),
            address = Address(address),
            already_suicided = already_suicided,
            balance_before_suicide = U256(balance_before_suicide),
        ).as_ref());
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

    fn record_before_call_gas_event(&mut self, gas_value: usize) {
        let call_index = self.active_call_index();
		let for_call_index = self.call_index + 1;
		self.gas_event_call_stack.push(for_call_index);
        // Matt: Validate against Geth logic, see commented code at top of this implementation
        self.printer.print(format!("GAS_EVENT {call_index} {for_call_index} {reason} {gas_value}",
            call_index = call_index,
            for_call_index = for_call_index,
            reason = "before_call",
            gas_value = gas_value as u64,
        ).as_ref());
    }

    fn record_after_call_gas_event(&mut self, gas_value: usize) {
		let call_index = self.active_call_index();
		let for_call_index = match self.gas_event_call_stack.pop() {
			Some(index) => index,
			None => panic!("There should always be a call in our gas event call start [{:?}]",self.hash)
		};

        self.printer.print(format!("GAS_EVENT {call_index} {for_call_index} {reason} {gas_value}",
            call_index = call_index,
            for_call_index = for_call_index,
            reason = "after_call",
            gas_value = gas_value  as u64,
        ).as_ref());
    }

    fn get_log_count(&self) -> u64 {
        self.log_count
    }

    fn debug(&mut self, message: String) {
		let active_call_index = self.active_call_index();
		let last_pop_call_index = self.last_pop_call_index.unwrap_or(0);

		self.printer.debug(format!("CONTEXT active_call_index={active_call_index} last_pop_call_index={last_pop_call_index} message={message}",
			active_call_index = active_call_index,
			last_pop_call_index = last_pop_call_index,
			message = message,
        ).as_ref());
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

    pub fn block_context(&self) -> BlockContext {
        BlockContext {
            context: self,
            is_enabled: self.is_enabled(),
            is_finalize_block_enabled: self.is_finalize_block_enabled(),
            cumulative_gas_used: 0,
            log_index_at_block: 0,
        }
    }

    pub fn block_tracer(&self) -> BlockTracer {
        BlockTracer{printer: self.printer.clone()}
    }

    pub fn is_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full;
    }

    pub fn is_finalize_block_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full || self.instrumentation == Instrumentation::BlockProgress;
    }

	pub fn init(&self, engine: String) {
		let platform_version = to_deepmind_version();
		self.printer.print(format!("INIT {protocol_major} {protocol_minor} {platform} {fork} {platform_major} {platform_minor} {platform_patch} {engine}",
			protocol_major = PROTOCOL_MAJOR,
			protocol_minor = PROTOCOL_MINOR,
			platform_major = platform_version.0,
			platform_minor = platform_version.1,
			platform_patch = platform_version.2,
			platform = PLATFORM,
			fork = FORK,
			engine = engine,
		).as_ref())
	}
}

pub struct BlockContext<'a> {
    context: &'a Context,
    is_enabled: bool,
    is_finalize_block_enabled: bool,
    cumulative_gas_used: u64,
    log_index_at_block: u64,
}

impl<'a> BlockContext<'a> {
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn is_finalize_block_enabled(&self) -> bool {
        self.is_finalize_block_enabled
    }

    pub fn start_block(&self, num: u64) {
        self.context.printer.print(format!("BEGIN_BLOCK {num}", num = num).as_ref())
    }

    pub fn transaction_tracer(&self, hash: eth::H256) -> TransactionTracer {
        TransactionTracer{
			hash: hash,
            printer: self.context.printer.clone(),
            call_index: 0,
			last_pop_call_index: None,
            call_stack: Vec::with_capacity(16),
			gas_event_call_stack: Vec::with_capacity(16),
            active_gas_left_at_failure: None,
            log_in_block_index: self.log_index_at_block,
            log_count: 0,
        }
    }

    pub fn start_transaction(&self, trx: Transaction) {
        let (v, r, s) = trx.signature;
        let mut to_str = ".".to_owned();
        if let Some(ref address) = trx.to {
            to_str = format!("{:x}", Address(address));
        }

        self.context.printer.print(format!("BEGIN_APPLY_TRX {hash:x} {to} {value:x} {v:x} {r:x} {s:x} {gas_limit} {gas_price:x} {nonce} {data:x}",
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

        self.context.printer.print(format!("TRX_FROM {from:x}", from = Address(&trx.from)).as_ref());
    }

    pub fn record_log_count(&mut self, count: u64) {
        self.log_index_at_block += count;
    }

    pub fn get_cumulative_gas_used(&mut self) -> u64 {
        self.cumulative_gas_used
    }

    pub fn set_cumulative_gas_used(&mut self, gas_used: u64) {
        self.cumulative_gas_used = gas_used;
    }

    pub fn end_transaction(&mut self, receipt: TransactionReceipt) {
        let mut post_state_bytes: &[u8] = &EMPTY_BYTES;
        if !receipt.post_state.is_zero() {
            post_state_bytes = receipt.post_state.as_bytes();
        }

        self.context.printer.print(format!("END_APPLY_TRX {gas_used} {post_state:x} {cumulative_gas_used} {logs_bloom:x} {logs}",
            gas_used = receipt.cumulative_gas_used - self.cumulative_gas_used,
            // Geth prints this as a Hex while it's really an Hash, let's be consistent with Geth here
            post_state = Hex(post_state_bytes),
            cumulative_gas_used = receipt.cumulative_gas_used,
            logs_bloom = Hex(receipt.logs_bloom),
            logs = serde_json::to_string(&receipt.logs).unwrap(),
        ).as_ref());

        self.cumulative_gas_used = receipt.cumulative_gas_used;
    }

    pub fn finalize_block(&self, num: u64) {
        self.context.printer.print(format!("FINALIZE_BLOCK {num}", num = num).as_ref())
    }

    pub fn end_block(&self, num: u64, size: u64, header:  Header, uncles: Vec<Header>) {
		self.context.printer.print(format!("END_BLOCK {num} {size} {meta}",
            num = num,
            size = size,
			meta = serde_json::to_string(&BlockEndMeta{header, uncles}).unwrap(),
        ).as_ref())
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

pub struct TransactionReceipt<'a> {
    pub cumulative_gas_used: u64,
    pub post_state: eth::H256,
    pub logs_bloom: &'a [u8],
    pub logs: Vec<Log<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Header<'a> {
	pub parent_hash: eth::H256,
	pub sha3_uncles: eth::H256,
	pub miner: eth::Address,
	pub state_root: eth::H256,
	pub transactions_root: eth::H256,
	pub receipts_root: eth::H256,
	pub logs_bloom: Hex<'a>,
	pub difficulty: eth::U256,
	pub number: U64,
	pub gas_limit: eth::U256,
	pub gas_used: eth::U256,

	pub timestamp: U64,
	pub extra_data: Hex<'a>,
	pub mix_hash: eth::H256,
	pub nonce: eth::H64,
	pub hash: eth::H256,
}

#[derive(Serialize)]
pub struct BlockEndMeta<'a> {
	pub header: Header<'a>,
	pub uncles: Vec<Header<'a>>
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Log<'a> {
    pub address: eth::Address,
	pub topics: &'a Vec<eth::H256>,
	pub data: Hex<'a>,
}

struct Address<'a>(&'a eth::Address);

impl fmt::LowerHex for Address<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.is_zero() {
            true => f.write_str("."),
            _ => fmt::LowerHex::fmt(self.0, f)
        }
    }
}

pub struct Hex<'a>(pub &'a [u8]);

impl fmt::LowerHex for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.len() {
            0 => f.write_str("."),
            _ => f.write_str(self.0.to_hex::<String>().as_ref()),
        }
    }
}

impl serde::Serialize for Hex<'_> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
	{
		serializer.serialize_str(&format!("0x{:}",self.0.to_hex::<String>()))
	}
}


struct H256<'a>(&'a eth::H256);

impl fmt::LowerHex for H256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(self.0, f)
    }
}

impl H256<'_> {
    pub fn to_hex(&self) -> String {
        self.0.as_bytes().to_hex::<String>()
    }
}

struct U256<'a>(&'a eth::U256);

impl fmt::LowerHex for U256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.is_zero() {
            true => f.write_str("."),
            _ => fmt::LowerHex::fmt(self.0, f),
        }
    }
}

pub struct U64(pub u64);

impl serde::Serialize for U64 {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: Serializer,
	{
		serializer.serialize_str(&format!("0x{:x}",self.0))
	}

}

