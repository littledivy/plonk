#[cfg_attr(feature = "hot_swap", export_name = "say_hello_new")]
#[no_mangle]
pub fn say_hello() {
    println!("Hello, world! x2");
}
