use super::big_int_api_mock::*;
use super::big_uint_api_mock::*;
use crate::TxContext;
use elrond_wasm::types::{Address, H256};

impl elrond_wasm::api::ContractHookApi<RustBigInt, RustBigUint> for TxContext {
	type Storage = Self;
	type CallValue = Self;
	type SendApi = Self;

	#[inline]
	fn get_storage_raw(&self) -> Self::Storage {
		self.clone()
	}

	#[inline]
	fn call_value(&self) -> Self::CallValue {
		self.clone()
	}

	#[inline]
	fn send(&self) -> Self::SendApi {
		self.clone()
	}

	fn get_sc_address(&self) -> Address {
		self.tx_input_box.to.clone()
	}

	fn get_owner_address(&self) -> Address {
		self.blockchain_info_box
			.contract_owner
			.clone()
			.unwrap_or_else(|| panic!("contract owner address not set"))
	}

	fn get_shard_of_address(&self, _address: &Address) -> u32 {
		panic!("get_shard_of_address not implemented")
	}

	fn is_smart_contract(&self, _address: &Address) -> bool {
		panic!("is_smart_contract not implemented")
	}

	fn get_caller(&self) -> Address {
		self.tx_input_box.from.clone()
	}

	fn get_balance(&self, address: &Address) -> RustBigUint {
		if address != &self.get_sc_address() {
			panic!("get balance not yet implemented for accounts other than the contract itself");
		}
		self.blockchain_info_box.contract_balance.clone().into()
	}

	fn get_tx_hash(&self) -> H256 {
		self.tx_input_box.tx_hash.clone()
	}

	fn get_gas_left(&self) -> u64 {
		self.tx_input_box.gas_limit
	}

	fn get_block_timestamp(&self) -> u64 {
		self.blockchain_info_box.current_block_info.block_timestamp
	}

	fn get_block_nonce(&self) -> u64 {
		self.blockchain_info_box.current_block_info.block_nonce
	}

	fn get_block_round(&self) -> u64 {
		self.blockchain_info_box.current_block_info.block_round
	}

	fn get_block_epoch(&self) -> u64 {
		self.blockchain_info_box.current_block_info.block_epoch
	}

	fn get_block_random_seed(&self) -> Box<[u8; 48]> {
		self.blockchain_info_box
			.current_block_info
			.block_random_seed
			.clone()
	}

	fn get_prev_block_timestamp(&self) -> u64 {
		self.blockchain_info_box.previous_block_info.block_timestamp
	}

	fn get_prev_block_nonce(&self) -> u64 {
		self.blockchain_info_box.previous_block_info.block_nonce
	}

	fn get_prev_block_round(&self) -> u64 {
		self.blockchain_info_box.previous_block_info.block_round
	}

	fn get_prev_block_epoch(&self) -> u64 {
		self.blockchain_info_box.previous_block_info.block_epoch
	}

	fn get_prev_block_random_seed(&self) -> Box<[u8; 48]> {
		self.blockchain_info_box
			.previous_block_info
			.block_random_seed
			.clone()
	}
}
