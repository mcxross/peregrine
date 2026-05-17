module model_context_pack::main;

public struct Vault has key, store {
    id: UID,
    owner: address,
    balance: u64,
}

public fun new(ctx: &mut TxContext): Vault {
    Vault { id: object::new(ctx), owner: tx_context::sender(ctx), balance: 0 }
}

public fun deposit(vault: &mut Vault, amount: u64, ctx: &mut TxContext) {
    assert!(vault.owner == tx_context::sender(ctx), 0);
    vault.balance = vault.balance + amount;
}
