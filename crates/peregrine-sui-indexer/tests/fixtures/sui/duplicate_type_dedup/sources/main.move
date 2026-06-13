module duplicate_type_dedup::main;

public struct Shared has copy, drop, store { value: u64 }

public fun first(value: Shared): u64 { value.value }
public fun second(value: Shared): u64 { value.value + 1 }
public fun combine(left: Shared, right: Shared): u64 { first(left) + second(right) }
