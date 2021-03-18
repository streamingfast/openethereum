// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Open Ethereum.

// Open Ethereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Open Ethereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Open Ethereum.  If not, see <http://www.gnu.org/licenses/>.

//! Evm input params.
use ethereum_types::{U256, H256, Address};
use bytes::Bytes;
use hash::{keccak, KECCAK_EMPTY};
use ethjson;

use action_type::ActionType;

use std::sync::Arc;

/// Transaction value
#[derive(Clone, Debug)]
pub enum ActionValue {
	/// Value that should be transfered
	Transfer(U256),
	/// Apparent value for transaction (not transfered)
	Apparent(U256)
}

/// Type of the way parameters encoded
#[derive(Clone, Debug)]
pub enum ParamsType {
	/// Parameters are included in code
	Embedded,
	/// Parameters are passed in data section
	Separate,
}

impl ActionValue {
	/// Returns action value as U256.
	pub fn value(&self) -> U256 {
		match *self {
			ActionValue::Transfer(x) | ActionValue::Apparent(x) => x
		}
	}

	/// Returns the transfer action value of the U256-convertable raw value
	pub fn transfer<T: Into<U256>>(transfer_value: T) -> ActionValue {
		ActionValue::Transfer(transfer_value.into())
	}

	/// Returns the apparent action value of the U256-convertable raw value
	pub fn apparent<T: Into<U256>>(apparent_value: T) -> ActionValue {
		ActionValue::Apparent(apparent_value.into())
	}

	/// Returns the transfer action value or None if only an apparent value
	pub fn to_deepmind_value(&self) -> Option<U256> {
		match *self {
			ActionValue::Transfer(amount) => Some(amount),
			_ => None,
		}
	}
}

// TODO: should be a trait, possible to avoid cloning everything from a Transaction(/View).
/// Action (call/create) input params. Everything else should be specified in Externalities.
#[derive(Clone, Debug)]
pub struct ActionParams {
	/// Address of currently executed code.
	pub code_address: Address,
	/// Hash of currently executed code.
	pub code_hash: Option<H256>,
	/// Receive address. Usually equal to code_address,
	/// except when called using CALLCODE.
	pub address: Address,
	/// Sender of current part of the transaction.
	pub sender: Address,
	/// Transaction initiator.
	pub origin: Address,
	/// Gas paid up front for transaction execution
	pub gas: U256,
	/// Gas price.
	pub gas_price: U256,
	/// Transaction value.
	pub value: ActionValue,
	/// Code being executed.
	pub code: Option<Arc<Bytes>>,
	/// Code version being executed.
	pub code_version: U256,
	/// Input data.
	pub data: Option<Bytes>,
	/// Type of action (e.g. CALL, DELEGATECALL, CREATE, etc.)
	pub action_type: ActionType,
	/// Param types encoding
	pub params_type: ParamsType,
}

impl Default for ActionParams {
	/// Returns default ActionParams initialized with zeros
	fn default() -> ActionParams {
		ActionParams {
			code_address: Address::zero(),
			code_hash: Some(KECCAK_EMPTY),
			address: Address::zero(),
			sender: Address::zero(),
			origin: Address::zero(),
			gas: U256::zero(),
			gas_price: U256::zero(),
			value: ActionValue::Transfer(U256::zero()),
			code: None,
			code_version: U256::zero(),
			data: None,
			action_type: ActionType::Create,
			params_type: ParamsType::Separate,
		}
	}
}

impl From<ethjson::vm::Transaction> for ActionParams {
	fn from(t: ethjson::vm::Transaction) -> Self {
		let address: Address = t.address.into();
		ActionParams {
			code_address: Address::zero(),
			code_hash: Some(keccak(&*t.code)),
			address: address,
			sender: t.sender.into(),
			origin: t.origin.into(),
			code: Some(Arc::new(t.code.into())),
			code_version: t.code_version.into(),
			data: Some(t.data.into()),
			gas: t.gas.into(),
			gas_price: t.gas_price.into(),
			value: ActionValue::Transfer(t.value.into()),
			action_type: ActionType::Call,
			params_type: ParamsType::Separate,
		}
	}
}

impl ActionParams {
	pub fn has_code_for_deepmind(&self) -> bool {
		// It appears that even for Genesis accounts, the code of the account is set
		// to 0 bytes when the genesis definition didn't had any code set. So just checking
		// the code option is not enough, the bytes lenght must be checked at the same time.
		match self.code {
			Some(ref bytes) if bytes.len() > 0 => true,
			_ => false
		}
	}

	pub fn to_deepmind_call(&self) -> deepmind::Call {
		deepmind::Call {
			call_type: self.action_type.to_deepmind_call_type(),
			from: self.sender,
			to: self.code_address,
			gas_limit: self.gas.as_u64(),
			value: self.value.to_deepmind_value(),
			input: self.data.as_ref().map(|value| value as &[u8]),
		}
	}
}
