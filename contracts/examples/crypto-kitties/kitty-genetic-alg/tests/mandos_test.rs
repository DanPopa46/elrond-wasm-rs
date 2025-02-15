extern crate kitty_genetic_alg;
use kitty_genetic_alg::*;

use elrond_wasm::*;
use elrond_wasm_debug::*;

fn contract_map() -> ContractMap<TxContext> {
	let mut contract_map = ContractMap::new();
	contract_map.register_contract(
		"file:../output/kitty-genetic-alg.wasm",
		Box::new(|context| Box::new(KittyGeneticAlgImpl::new(context))),
	);
	contract_map
}

#[test]
fn init() {
	parse_execute_mandos("mandos/init.scen.json", &contract_map());
}

#[test]
fn generate_kitty_genes_test() {
	parse_execute_mandos("mandos/generate-kitty-genes.scen.json", &contract_map());
}
