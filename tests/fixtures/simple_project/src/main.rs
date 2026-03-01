use crate::lib::old_function;

fn main() {
    println!("Hello from old_function!");
    let result = old_function(42);
    println!("Result: {result}");
    old_helper();
}

fn old_helper() {
    println!("I am old_helper");
}

fn another_function() {
    // This function should not be renamed
    println!("another_function is fine");
}
