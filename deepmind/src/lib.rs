extern crate common_types as types;
extern crate ethereum_types;
extern crate rustc_hex;

use std::{borrow::Borrow, fmt::{self, LowerHex}, sync::atomic::{AtomicBool, Ordering}};
use rustc_hex::{ToHex};
use types::{BlockNumber, header::Header, transaction::{Action, SignedTransaction, UnverifiedTransaction}};
use vm::{ActionParams, ActionType, ActionValue, ReturnData};

// This set of variables/functions are unused for now, will see how far we need them or not
static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed)
}

pub fn disable() {
    ENABLED.store(false, Ordering::Relaxed)
}

pub fn is_enabled() -> bool {
    return ENABLED.load(Ordering::Relaxed);
}
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
    fn print(&self, input: &str);
}

pub struct DiscardPrinter {
}

impl Printer for DiscardPrinter {
    fn print(&self, _input: &str) {}
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

#[derive(Debug, PartialEq, Clone)]
pub enum Instrumentation {
    Full,
    BlockProgress,
    None,
}

pub struct Context {
    instrumentation: Instrumentation,
    printer: Box<dyn Printer>,
}

impl Context {
    pub fn new(instrumentation: Instrumentation) -> Context {
        Context {
            instrumentation,
            // printer: Box::new(IoPrinter{io: Box::new(std::io::stdout())}),
            printer: Box::new(IoPrinter{}),
        }
    }

    pub fn noop() -> Context {
        Context {
            instrumentation: Instrumentation::None,
            printer: Box::new(DiscardPrinter{}),
        }
    }

    pub fn is_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full;
    }

    pub fn is_finalize_block_enabled(&self) -> bool {
        return self.instrumentation == Instrumentation::Full || self.instrumentation == Instrumentation::BlockProgress;
    }

    pub fn start_block(&self, num: BlockNumber) {
        self.printer.print(format!("BEGIN_BLOCK {num}", num = num).as_ref())
    }

    pub fn finalize_block(&self, num: BlockNumber) {
        self.printer.print(format!("FINALIZE_BLOCK {num}", num = num).as_ref())
    }

    pub fn end_block(&self, size: u64, header: &Header, _uncles: &Vec<Header>) {
        self.printer.print(format!("END_BLOCK {num} {size}",
            num = header.number(),
            size = size,
        ).as_ref())
    }

    pub fn start_transaction(&self, t: &SignedTransaction) {
        let (v, r, s) = t.signature_for_deepmind();
        let trx = t.as_unsigned();
        let mut to = ".".to_owned();
        if let Action::Call(ref address) = trx.action {
            to = format!("{:x}", Address(&address));
        }

        self.printer.print(format!("BEGIN_APPLY_TRX {hash:x} {to} {value:x} {v:x} {r:x} {s:x} {gas_limit} {gas_price:x} {nonce} {data:x}",
            hash = H256(&t.hash()),
            to = to,
            value = U256(&trx.value),
            v = v,
            r = H256(&r),
            s = H256(&s),
            gas_limit = trx.gas.as_u64(),
            gas_price = U256(&trx.gas_price),
            nonce = trx.nonce,
            data = Hex(&trx.data),
        ).as_ref());

        self.printer.print(format!("TRX_FROM {from:x}", from = Address(&t.sender())).as_ref());
    }

    pub fn end_transaction(&self) {
        self.printer.print(format!("END_TRX").as_ref())
    }

    pub fn start_call(&self, params: &ActionParams) {
        self.printer.print(format!("EVM_RUN_CALL {call_type} {call_index}",
            call_type = CallType(&params.action_type),
            call_index = 0,
        ).as_ref());

        let mut value = ".".to_owned();
        if let ActionValue::Transfer(ref amount) = params.value {
            value = format!("{:x}", U256(amount));
        }

        let mut input = ".".to_owned();
        if let Some(ref bytes) = params.data {
            input = format!("{:x}", Hex(bytes));
        }

        self.printer.print(format!("EVM_PARAM {call_type} {call_index} {from:x} {to:x} {value} {gas_limit} {input}",
            call_type = CallType(&params.action_type),
            call_index = 0,
            from = Address(&params.sender),
            to = Address(&params.address),
            value = value,
            gas_limit = params.gas.as_u64(),
            input = input,
        ).as_ref());
    }

    pub fn revert_call(&self) {
        self.printer.print(format!("EVM_REVERTED {call_index}",
            call_index = 0,
        ).as_ref());
    }

    pub fn end_call(&self, gas_left: &ethereum_types::U256, return_data: &vm::ReturnData ) {
        let bytes: &[u8]= return_data;
        let mut return_value = ".".to_owned();
        if bytes.len() > 0 {
            return_value = format!("{:x}", Hex(bytes));
        }

        self.printer.print(format!("EVM_END_CALL {call_index} {gas_left:} {return_value}",
            call_index = 0,
            gas_left = gas_left.as_u64(),
            return_value = return_value,
        ).as_ref());
    }

    pub fn end_failed_call(&self, gas_left: &ethereum_types::U256, err: &vm::Error) {
        self.printer.print(format!("EVM_CALL_FAILED {call_index} {gas_left} {reason}",
            call_index = 0,
            gas_left = gas_left.as_u64(),
            reason = err,
        ).as_ref());

        self.end_call(gas_left, &ReturnData::empty())
    }
}

struct Address<'a>(&'a ethereum_types::Address);

impl fmt::LowerHex for Address<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}

struct CallType<'a>(&'a ActionType);

impl fmt::Display for CallType<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
       let type_name = match self.0 {
           ActionType::Call => "CALL",
           ActionType::CallCode => "CALLCODE",
           ActionType::Create => "CREATE",
           ActionType::Create2 => "CREATE",
           ActionType::DelegateCall => "DELEGATE",
           ActionType::StaticCall => "STATIC",
       };

       return f.write_str(type_name)
    }
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

struct H256<'a>(&'a ethereum_types::H256);

impl fmt::LowerHex for H256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}

struct U256<'a>(&'a ethereum_types::U256);

impl fmt::LowerHex for U256<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return f.write_str(".")
        }

        fmt::LowerHex::fmt(self.0, f)
    }
}