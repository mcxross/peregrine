module friend_function::b {
    public fun call_friend(): u64 { friend_function::a::friend_only() }
}
