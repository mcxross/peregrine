module bytecode_fixture::vault;

public struct Vault has key, store {
    id: UID,
    owner: address,
    balance: u64,
}

public fun create(ctx: &mut TxContext): Vault {
    let sender = tx_context::sender(ctx);
    Vault { id: object::new(ctx), owner: sender, balance: 0 }
}

public fun deposit(vault: &mut Vault, amount: u64, ctx: &mut TxContext) {
    let sender = tx_context::sender(ctx);
    assert!(vault.owner == sender, 0);
    vault.balance = vault.balance + amount;
}

public fun send(vault: Vault, recipient: address) {
    transfer::public_transfer(vault, recipient);
}

public fun wrapper(vault: Vault, recipient: address) {
    send(vault, recipient);
}

public fun transfer_then_assert(vault: Vault, recipient: address) {
    transfer::public_transfer(vault, recipient);
    assert!(recipient != @0x0, 1);
}
