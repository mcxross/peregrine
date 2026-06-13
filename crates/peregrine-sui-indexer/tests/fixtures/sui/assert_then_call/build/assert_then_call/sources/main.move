module assert_then_call::main;

public struct Obj has key, store {
    id: UID,
    owner: address,
}

public fun new(ctx: &mut TxContext): Obj {
    Obj { id: object::new(ctx), owner: tx_context::sender(ctx) }
}

public fun check_then_send(obj: Obj, recipient: address, ctx: &mut TxContext) {
    assert!(obj.owner == tx_context::sender(ctx), 0);
    transfer::public_transfer(obj, recipient);
}
