module dynamic_field_usage::main;

use sui::dynamic_field;

public struct Parent has key, store { id: UID }
public struct Child has store { value: u64 }

public fun new(ctx: &mut TxContext): Parent { Parent { id: object::new(ctx) } }

public fun add_child(parent: &mut Parent, name: vector<u8>, value: u64) {
    dynamic_field::add(&mut parent.id, name, Child { value });
}

public fun read_child(parent: &Parent, name: vector<u8>): u64 {
    let child = dynamic_field::borrow<vector<u8>, Child>(&parent.id, name);
    child.value
}
