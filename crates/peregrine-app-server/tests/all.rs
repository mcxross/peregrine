#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::match_like_matches_macro,
    clippy::redundant_clone,
    clippy::too_many_arguments,
    clippy::result_large_err
)]
// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/suite/`.
mod suite;
