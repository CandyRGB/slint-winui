#[path = "../src/behavior/number_box.rs"]
mod number_box;

slint::slint! {
    import { NumberBoxExample, NumberBoxMath, NumberBoxParseResult, NumberBoxValue }
        from "ui/number_box.slint";
    export { NumberBoxExample, NumberBoxMath, NumberBoxParseResult, NumberBoxValue }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = NumberBoxExample::new()?;
    number_box::install(window.global::<NumberBoxMath>());
    window.set_dark(std::env::args().any(|argument| argument == "--dark"));
    window.run()
}
