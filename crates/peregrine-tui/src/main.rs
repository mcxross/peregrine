#![allow(clippy::filter_next, clippy::module_inception, clippy::explicit_counter_loop, clippy::needless_return, clippy::uninlined_format_args, clippy::derivable_impls, clippy::large_enum_variant, clippy::result_large_err)]
#![allow(clippy::unnecessary_to_owned, clippy::too_many_arguments, clippy::collapsible_if)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_imports, clippy::match_like_matches_macro)]
fn main() {
    match peregrine_tui::run() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
