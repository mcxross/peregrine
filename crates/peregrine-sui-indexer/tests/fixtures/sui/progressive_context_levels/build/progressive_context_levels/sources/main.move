module progressive_context_levels::main;

public fun entry(): u64 { a() }
fun a(): u64 { b() + 1 }
fun b(): u64 { c() + 1 }
fun c(): u64 { 1 }
