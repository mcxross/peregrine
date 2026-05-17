module entry_function::main;

public struct Counter has key, store {
    id: UID,
    value: u64,
}

public fun new(ctx: &mut TxContext): Counter {
    Counter { id: object::new(ctx), value: 0 }
}

public entry fun increment(counter: &mut Counter, amount: u64, ctx: &mut TxContext) {
    let _sender = tx_context::sender(ctx);
    counter.value = counter.value + amount;
}
