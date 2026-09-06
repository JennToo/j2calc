use embassy_time::Timer;
use embedded_graphics::{
    Drawable, Pixel,
    geometry::{Dimensions, Point, Size},
    image::{Image, ImageRaw},
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
    text::{Alignment, Text},
};
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_hal_1::digital::OutputPin;
use embedded_hal_async::spi::SpiBus;

const WIDTH: usize = 400;
const HEIGHT: usize = 240;

const WRITE_BIT: u8 = 0b10000000;
const VCOM_BIT: u8 = 0b01000000;
const CLEAR_BIT: u8 = 0b00100000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Spi,
    Gpio,
}

pub struct Lcd<SPI: SpiBus<u8>, CS: OutputPin> {
    spi: SPI,
    cs: CS,
    vcom: bool,
    framebuffer: [[u8; WIDTH / 8]; HEIGHT],
}

impl<SPI: SpiBus<u8>, CS: OutputPin> Lcd<SPI, CS> {
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self {
            spi,
            cs,
            vcom: false,
            framebuffer: [[0xFF; WIDTH / 8]; HEIGHT],
        }
    }

    pub async fn update_vcom(&mut self) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::Gpio)?;
        Timer::after_micros(1).await;

        let cmd = [self.toggle_vcom(), 0u8];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;

        self.cs.set_low().map_err(|_| Error::Gpio)?;
        self.vcom = !self.vcom;

        Ok(())
    }

    pub async fn clear_screen(&mut self) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::Gpio)?;
        Timer::after_micros(1).await;

        let cmd = [self.toggle_vcom() | CLEAR_BIT, 0u8];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;

        self.cs.set_low().map_err(|_| Error::Gpio)?;

        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::Gpio)?;
        Timer::after_micros(1).await;

        // TODO: Be smarter and only update lines that changed, could save power
        let cmd = [self.toggle_vcom() | WRITE_BIT, 0u8];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;

        for line in 0..HEIGHT {
            self.spi
                .write(&self.framebuffer[line])
                .await
                .map_err(|_| Error::Spi)?;
            let cmd = [0u8, ((line + 1) as u8).reverse_bits()];
            self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;
        }

        self.cs.set_low().map_err(|_| Error::Gpio)?;

        Ok(())
    }

    fn toggle_vcom(&mut self) -> u8 {
        self.vcom = !self.vcom;
        if self.vcom { VCOM_BIT } else { 0u8 }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: BinaryColor) {
        if x >= WIDTH as i32 || y >= HEIGHT as i32 || x < 0 || y < 0 {
            return;
        }

        let bit: u8 = 1u8 << (7 - (x % 8));
        let value = if color == BinaryColor::Off { bit } else { 0u8 };
        let mask = !bit;
        let cell = &mut self.framebuffer[y as usize][x as usize / 8];
        *cell = (*cell & mask) | value;
    }
}

impl<SPI: SpiBus<u8>, CS: OutputPin> Dimensions for Lcd<SPI, CS> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle {
            top_left: Point::zero(),
            size: Size::new(WIDTH as u32, HEIGHT as u32),
        }
    }
}

impl<SPI: SpiBus<u8>, CS: OutputPin> DrawTarget for Lcd<SPI, CS> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        pixels
            .into_iter()
            .for_each(|p| self.set_pixel(p.0.x, p.0.y, p.1));
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let value = if color == BinaryColor::Off { 0xFF } else { 0 };
        self.framebuffer = [[value; WIDTH / 8]; HEIGHT];
        Ok(())
    }
}

const INTRO: &[&str] = textwrap_macros::wrap!(
    "Did you ever hear the tragedy of Darth Plagueis the Wise? I thought not. It's not a story the Jedi would tell you. It's a Sith legend. Darth Plagueis was a Dark Lord of the Sith, so powerful and so wise he could use the Force to influence the midichlorians to create life... He had such a knowledge of the dark side that he could even keep the ones he cared about from dying. The dark side of the Force is a pathway to many abilities some consider to be unnatural. He became so powerful... the only thing he was afraid of was losing his power, which eventually, of course, he did. Unfortunately, he taught his apprentice everything he knew, then his apprentice killed him in his sleep. It's ironic he could save others from death, but not himself.",
    40
);
const IMAGE_DATA: &[u8] = include_bytes!("../Darth.bin");
pub fn draw_splash<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    y_offset: i32,
) -> Result<(), D::Error> {
    display.clear(BinaryColor::Off)?;

    let raw_image = ImageRaw::<BinaryColor>::new(IMAGE_DATA, 400);

    let image = Image::new(&raw_image, Point::new(0, y_offset));
    image.draw(display)?;

    let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    // Text::with_alignment(
    //     "j2calc",
    //     Point::new((WIDTH / 2) as i32, y_offset),
    //     character_style,
    //     Alignment::Center,
    // )
    // .draw(display)?;

    let mut line_y = y_offset + 240;
    for line in INTRO {
        Text::with_alignment(
            line,
            Point::new(0, line_y),
            character_style,
            Alignment::Left,
        )
        .draw(display)?;
        line_y += 20;
    }

    Ok(())
}
