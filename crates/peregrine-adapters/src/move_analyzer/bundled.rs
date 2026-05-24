use super::{MoveAnalyzerAdapterSource, MoveAnalyzerAdapterSourceStatus};
use move_compiler::editions::Flavor;
use sui_package_alt::SuiFlavor;

pub(crate) fn status() -> MoveAnalyzerAdapterSourceStatus {
    MoveAnalyzerAdapterSourceStatus {
        source: MoveAnalyzerAdapterSource::BundledLibrary,
        available: true,
        version: None,
        path: None,
        error: None,
    }
}

pub fn run_stdio() {
    move_analyzer::analyzer::run::<SuiFlavor>(Some(Flavor::Sui));
}
