module clock_usage::main;

use sui::clock::{Self, Clock};

public fun now_ms(clock: &Clock): u64 {
    clock::timestamp_ms(clock)
}
