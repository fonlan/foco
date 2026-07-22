pub mod formatting;

use crate::formatting::decorate as display;

pub fn local_helper(input: &str) -> String {
    display(input)
}

pub fn render(input: &str) -> String {
    let result = local_helper(input);
    result
}

pub fn same_name() {}

pub fn call_outer() {
    same_name();
}

mod nested {
    pub fn same_name() {}

    pub fn call_inner() {
        same_name();
    }
}
