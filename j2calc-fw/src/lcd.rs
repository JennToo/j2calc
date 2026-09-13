use core::fmt;

use embassy_time::Timer;
use embedded_graphics::{
    Drawable, Pixel,
    geometry::{Dimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
    text::Text,
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
    text_cursor: Point,
}

impl BinaryFramebuffer {
    pub fn new() -> Self {
        Self {
            data: [[0xFF; WIDTH / 8]; HEIGHT],
            text_cursor: Point::new(0, 120),
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
        self.text_cursor = Point::new(0, 120);
        Ok(())
    }
}

const TEXT_WIDTH: usize = WIDTH / 10;
const TEXT_HEIGHT: usize = HEIGHT / 20;
pub struct TextFramebuffer {
    data: [[char; TEXT_WIDTH]; TEXT_HEIGHT],
    cursor: (usize, usize),
}

impl TextFramebuffer {
    pub fn new() -> Self {
        Self {
            data: [[' '; TEXT_WIDTH]; TEXT_HEIGHT],
            cursor: (0, 0),
        }
    }

    pub fn clear(&mut self) {
        self.data = [[' '; TEXT_WIDTH]; TEXT_HEIGHT];
        self.cursor = (0, 0);
    }

    pub fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let character_style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let mut cursor = Point::new(0, 20);
        for line in 0..TEXT_HEIGHT {
            for col in 0..TEXT_WIDTH {
                let mut mem = [0; 4];
                let s = self.data[line][col].encode_utf8(&mut mem);
                Text::new(s, cursor, character_style).draw(target)?;
                cursor.x += 10;
            }
            cursor.x = 0;
            cursor.y += 20;
        }
        Ok(())
    }
}

impl fmt::Write for TextFramebuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for char in s.chars() {
            if char == '\n' {
                self.cursor.1 += 1;
            } else {
                if self.cursor.0 < TEXT_WIDTH && self.cursor.1 < TEXT_HEIGHT {
                    self.data[self.cursor.1][self.cursor.0] = char;
                }

                if self.cursor.0 + 1 >= TEXT_WIDTH {
                    self.cursor.0 = 0;
                    self.cursor.1 += 1;
                } else {
                    self.cursor.0 += 1;
                }
            }
        }
        Ok(())
    }
}
