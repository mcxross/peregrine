module transfer_call_direct::main;

public struct Obj has key, store { id: UID }

public fun new(ctx: &mut TxContext): Obj { Obj { id: object::new(ctx) } }

public fun send(obj: Obj, recipient: address) {
    transfer::public_transfer(obj, recipient);
}
