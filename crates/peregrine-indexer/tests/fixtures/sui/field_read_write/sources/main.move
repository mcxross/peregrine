module field_read_write::main;

public struct Vault has key, store {
    id: UID,
    balance: u64,
}

public fun new(ctx: &mut TxContext): Vault {
    Vault { id: object::new(ctx), balance: 0 }
}

public fun add(vault: &mut Vault, amount: u64) {
    vault.balance = vault.balance + amount;
}
