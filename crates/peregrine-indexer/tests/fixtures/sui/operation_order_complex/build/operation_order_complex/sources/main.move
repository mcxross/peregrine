module operation_order_complex::main;

public fun branchy(input: u64): u64 {
    let mut value = input + 1;
    if (value > 10) {
        abort 7
    } else {
        value = value + 2;
    };
    value
}
