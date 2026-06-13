module struct_abilities::main;

public struct KeyObject has key, store {
    id: UID,
    value: u64,
}

public struct CopyDropStore has copy, drop, store {
    value: u64,
}

public fun new(ctx: &mut TxContext): KeyObject {
    KeyObject { id: object::new(ctx), value: 0 }
}
