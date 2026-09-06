use embassy_time::Timer;
use embedded_graphics::{
    Drawable, Pixel,
    geometry::{Dimensions, Point, Size},
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
}

impl<SPI: SpiBus<u8>, CS: OutputPin> Lcd<SPI, CS> {
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self {
            spi,
            cs,
            vcom: false,
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

    pub async fn draw_framebuffer(&mut self, framebuffer: &BinaryFramebuffer) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::Gpio)?;
        Timer::after_micros(1).await;

        // TODO: Be smarter and only update lines that changed, could save power
        let cmd = [self.toggle_vcom() | WRITE_BIT, 0u8];
        self.spi.write(&cmd).await.map_err(|_| Error::Spi)?;

        for line in 0..HEIGHT {
            self.spi
                .write(&framebuffer.data[line])
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
}

pub struct BinaryFramebuffer {
    data: [[u8; WIDTH / 8]; HEIGHT],
}

impl BinaryFramebuffer {
    pub fn new() -> Self {
        Self {
            data: [[0xFF; WIDTH / 8]; HEIGHT],
        }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: BinaryColor) {
        if x >= WIDTH as i32 || y >= HEIGHT as i32 || x < 0 || y < 0 {
            return;
        }

        let bit: u8 = 1u8 << (7 - (x % 8));
        let value = if color == BinaryColor::Off { bit } else { 0u8 };
        let mask = !bit;
        let cell = &mut self.data[y as usize][x as usize / 8];
        *cell = (*cell & mask) | value;
    }
}

impl Dimensions for BinaryFramebuffer {
    fn bounding_box(&self) -> Rectangle {
        Rectangle {
            top_left: Point::zero(),
            size: Size::new(WIDTH as u32, HEIGHT as u32),
        }
    }
}

impl DrawTarget for BinaryFramebuffer {
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
        self.data = [[value; WIDTH / 8]; HEIGHT];
        Ok(())
    }
}

pub fn draw_splash<D: DrawTarget<Color = BinaryColor>>(display: &mut D) -> Result<(), D::Error> {
    display.clear(BinaryColor::Off)?;
    let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
    Text::with_alignment(
        "j2calc",
        Point::new((WIDTH / 2) as i32, 120),
        character_style,
        Alignment::Center,
    )
    .draw(display)?;

    Ok(())
}
