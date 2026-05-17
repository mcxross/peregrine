module simple_module::main;

public struct Counter has key, store {
    id: UID,
    value: u64,
}

public fun new(ctx: &mut TxContext): Counter {
    Counter { id: object::new(ctx), value: 0 }
}

public fun value(counter: &Counter): u64 {
    counter.value
}
