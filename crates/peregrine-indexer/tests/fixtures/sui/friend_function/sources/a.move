module friend_function::a {
    friend friend_function::b;

    public(friend) fun friend_only(): u64 { 1 }
    public fun public_call(): u64 { friend_only() }
}
