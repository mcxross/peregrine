module pack_unpack::main;

public struct Pair has drop {
    left: u64,
    right: u64,
}

public fun make(left: u64, right: u64): Pair {
    Pair { left, right }
}

public fun destroy(pair: Pair): u64 {
    let Pair { left, right } = pair;
    left + right
}
