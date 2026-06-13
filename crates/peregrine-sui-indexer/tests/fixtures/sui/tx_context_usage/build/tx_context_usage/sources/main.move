module tx_context_usage::main;

public fun sender(ctx: &mut TxContext): address {
    tx_context::sender(ctx)
}
