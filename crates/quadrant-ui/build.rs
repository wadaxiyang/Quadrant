//! Builds the root Slint component graph.

fn main() {
    slint_build::compile("../../ui/app.slint").expect("failed to compile the Quadrant Slint UI");
}
