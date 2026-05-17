module call_graph_budget::main;

public fun entry(): u64 { a() }
fun a(): u64 { b() }
fun b(): u64 { c() }
fun c(): u64 { d() }
fun d(): u64 { e() }
fun e(): u64 { 5 }
