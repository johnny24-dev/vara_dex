use dex_test_transfer_io::ContractMetadata;

fn main() {
    gear_wasm_builder::build_with_metadata::<ContractMetadata>();
}
