module generic_function::main;

public struct Holder<T: store> has store {
    value: T,
}

public fun wrap<T: store>(value: T): Holder<T> {
    Holder<T> { value }
}

public fun unwrap_copy<T: copy + drop + store>(holder: &Holder<T>): T {
    holder.value
}
