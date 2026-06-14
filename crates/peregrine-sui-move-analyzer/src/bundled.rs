use move_compiler::editions::Flavor;
use sui_package_alt::SuiFlavor;

pub fn run_stdio() {
    move_analyzer::analyzer::run::<SuiFlavor>(Some(Flavor::Sui));
}
