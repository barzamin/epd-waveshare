#![deny(warnings)]

use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text, TextStyleBuilder},
};
use embedded_hal::prelude::*;
// use embedded_hal::prelude::*;
use epd_waveshare::{
    color::*,
    epd2in13_v2::{Display2in13, Epd2in13},
    graphics::DisplayRotation,
    prelude::*,
};
use linux_embedded_hal::{CdevPin, Delay, Spidev, gpio_cdev::{Chip, LineRequestFlags}, spidev::{self, SpidevOptions}};

// activate spi, gpio in raspi-config
// needs to be run with sudo because of some sysfs_gpio permission problems and follow-up timing problems
// see https://github.com/rust-embedded/rust-sysfs-gpio/issues/5 and follow-up issues

fn main() -> Result<(), std::io::Error> {
    // Configure SPI
    // Settings are taken from
    let mut spi = Spidev::open("/dev/spidev0.0").expect("spidev directory");
    let options = SpidevOptions::new()
        .bits_per_word(8)
        .max_speed_hz(4_000_000)
        .mode(spidev::SpiModeFlags::SPI_MODE_0)
        .build();
    spi.configure(&options).expect("spi configuration");

    // Configure Digital I/O Pin to be used as Chip Select for SPI
    let mut chip = Chip::new("/dev/gpiochip0").expect("chip");
    let cs = CdevPin::new(
        chip.get_line(8)
            .expect("cs line")
            .request(LineRequestFlags::OUTPUT, 1, "cs export")
            .expect("cs request"),
    ).expect("cs pin");

    let busy = CdevPin::new(
        chip.get_line(24)
            .expect("busy line")
            .request(LineRequestFlags::INPUT, 0, "busy export")
            .expect("busy request"),
    ).expect("busy pin");

    let dc = CdevPin::new(
        chip.get_line(25)
            .expect("dc line")
            .request(LineRequestFlags::OUTPUT, 1, "dc export")
            .expect("dc request"),
    ).expect("dc pin");

    let rst = CdevPin::new(
        chip.get_line(17)
            .expect("rst line")
            .request(LineRequestFlags::OUTPUT, 1, "rst export")
            .expect("rst request"),
    ).expect("rst pin");

    let mut delay = Delay {};

    let mut epd2in13 =
        Epd2in13::new(&mut spi, cs, busy, dc, rst, &mut delay).expect("eink initalize error");

    //println!("Test all the rotations");
    let mut display = Display2in13::default();

    display.set_rotation(DisplayRotation::Rotate0);
    draw_text(&mut display, "Rotate 0!", 5, 50);

    display.set_rotation(DisplayRotation::Rotate90);
    draw_text(&mut display, "Rotate 90!", 5, 50);

    display.set_rotation(DisplayRotation::Rotate180);
    draw_text(&mut display, "Rotate 180!", 5, 50);

    display.set_rotation(DisplayRotation::Rotate270);
    draw_text(&mut display, "Rotate 270!", 5, 50);

    epd2in13.update_frame(&mut spi, &display.buffer(), &mut delay).expect("update frame");
    epd2in13
        .display_frame(&mut spi, &mut delay)
        .expect("display frame new graphics");
    delay.try_delay_ms(5000u16).expect("delay");

    //println!("Now test new graphics with default rotation and some special stuff:");
    display.clear_buffer(Color::White);

    // draw a analog clock
    let _ = Circle::with_center(Point::new(64, 64), 80)
        .into_styled(PrimitiveStyle::with_stroke(Black, 1))
        .draw(&mut display);
    let _ = Line::new(Point::new(64, 64), Point::new(30, 40))
        .into_styled(PrimitiveStyle::with_stroke(Black, 4))
        .draw(&mut display);
    let _ = Line::new(Point::new(64, 64), Point::new(80, 40))
        .into_styled(PrimitiveStyle::with_stroke(Black, 1))
        .draw(&mut display);

    // draw white on black background
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
        .text_color(White)
        .background_color(Black)
        .build();
    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

    let _ = Text::with_text_style("It's working-WoB!", Point::new(90, 10), style, text_style)
        .draw(&mut display);

    // use bigger/different font
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
        .text_color(White)
        .background_color(Black)
        .build();

    let _ = Text::with_text_style("It's working\nWoB!", Point::new(90, 40), style, text_style)
        .draw(&mut display);

    // Demonstrating how to use the partial refresh feature of the screen.
    // Real animations can be used.
    epd2in13
        .set_refresh(&mut spi, &mut delay, RefreshLut::Quick)
        .unwrap();
    epd2in13.clear_frame(&mut spi, &mut delay).unwrap();

    // a moving `Hello World!`
    let limit = 10;
    for i in 0..limit {
        draw_text(&mut display, "  Hello World! ", 5 + i * 12, 50);

        epd2in13
            .update_and_display_frame(&mut spi, &display.buffer(), &mut delay)
            .expect("display frame new graphics");
        delay.try_delay_ms(1_000u16).expect("delay");
    }

    // Show a spinning bar without any delay between frames. Shows how «fast»
    // the screen can refresh for this kind of change (small single character)
    display.clear_buffer(Color::White);
    epd2in13
        .update_and_display_frame(&mut spi, &display.buffer(), &mut delay)
        .unwrap();

    let spinner = ["|", "/", "-", "\\"];
    for i in 0..10 {
        display.clear_buffer(Color::White);
        draw_text(&mut display, spinner[i % spinner.len()], 10, 100);
        epd2in13
            .update_and_display_frame(&mut spi, &display.buffer(), &mut delay)
            .unwrap();
    }

    println!("Finished tests - going to sleep");
    epd2in13.sleep(&mut spi, &mut delay).expect("sleep");

    Ok(())
}

fn draw_text(display: &mut Display2in13, text: &str, x: i32, y: i32) {
    let style = MonoTextStyleBuilder::new()
        .font(&embedded_graphics::mono_font::ascii::FONT_6X10)
        .text_color(White)
        .background_color(Black)
        .build();

    let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

    let _ = Text::with_text_style(text, Point::new(x, y), style, text_style).draw(display);
}
