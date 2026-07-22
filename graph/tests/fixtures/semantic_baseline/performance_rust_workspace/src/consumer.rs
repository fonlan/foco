use crate::generated::{step_00, step_20, step_40};

pub fn render_batch(value: usize) -> usize {
    step_40(step_20(step_00(value)))
}
