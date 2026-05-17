module transfer_then_assert::main;

public struct Obj has key, store { id: UID }

public fun new(ctx: &mut TxContext): Obj { Obj { id: object::new(ctx) } }

public fun transfer_then_check(obj: Obj, recipient: address) {
    transfer::public_transfer(obj, recipient);
    assert!(recipient != @0x0, 1);
}
