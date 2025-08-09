#![no_std]
#![no_main]

extern crate alloc;

use libtinyos::{exit, println};
use tinygraphics::{
    backend::{GraphicsBackend, PrimitiveDrawer},
    mono_font::{self, MonoTextStyleBuilder},
    pixelcolor::Rgb888,
    prelude::{Point, Primitive, RgbColor},
    primitives::PrimitiveStyleBuilder,
    text::Text,
};

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    let mut graphics_backend = PrimitiveDrawer::default();

    let glyph = tinygraphics::primitives::Circle::new(Point::new(500, 100), 42);
    let style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb888::RED)
        .stroke_width(4)
        .build();
    graphics_backend
        .draw_primitive(&glyph.into_styled(style))
        .unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&mono_font::ascii::FONT_10X20)
        .text_color(Rgb888::WHITE)
        .background_color(Rgb888::MAGENTA)
        .underline()
        .build();
    let text = Text::new(
        "Hello World from example-rs graphics!",
        Point::new(500, 200),
        text_style,
    );
    graphics_backend.draw_primitive(&text).unwrap();
    graphics_backend.flush().unwrap();

    println!("Hello World from example-rs");
    exit(0);
}
