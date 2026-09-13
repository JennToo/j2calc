use embassy_time::Timer;
use embedded_hal_async::{digital::Wait, spi::SpiDevice};

const CTRL_HDR: u8 = 0b01000000;
const CTRL_ADDR: u8 = 0b00000000;
const CTRL_READ: u8 = CTRL_HDR | CTRL_ADDR | 0b00000001;
const CTRL_WRITE: u8 = CTRL_HDR | CTRL_ADDR;

// BANK=0 registers
const REG_IODIRA: u8 = 0x00;
const REG_IODIRB: u8 = 0x01;
const REG_IPOLA: u8 = 0x02;
const REG_IPOLB: u8 = 0x03;
const REG_GPINTENA: u8 = 0x04;
const REG_GPINTENB: u8 = 0x05;
const REG_DEFVALA: u8 = 0x06;
const REG_DEFVALB: u8 = 0x07;
const REG_INTCONA: u8 = 0x08;
const REG_INTCONB: u8 = 0x09;
const REG_IOCON: u8 = 0x0A;
const REG_GPPUA: u8 = 0x0C;
const REG_GPPUB: u8 = 0x0D;
const REG_INTFA: u8 = 0x0E;
const REG_INTFB: u8 = 0x0F;
const REG_INTCAPA: u8 = 0x10;
const REG_INTCAPB: u8 = 0x11;
const REG_GPIOA: u8 = 0x12;
const REG_GPIOB: u8 = 0x13;
const REG_OLATA: u8 = 0x14;
const REG_OLATB: u8 = 0x15;

#[rustfmt::skip]
const STARTUP_SEQUENCE: &[[u8; 3]] = &[
    [CTRL_WRITE, REG_IOCON, 0x42],
    [CTRL_WRITE, REG_IODIRA, 0x00],
    [CTRL_WRITE, REG_IODIRB, 0xFF],
    [CTRL_WRITE, REG_IPOLA, 0x00],
    [CTRL_WRITE, REG_IPOLB, 0x00],
    [CTRL_WRITE, REG_INTCONA, 0x00],
    [CTRL_WRITE, REG_INTCONB, 0x7F],
    [CTRL_WRITE, REG_GPINTENA, 0x00],
    [CTRL_WRITE, REG_GPINTENB, 0x7F],
    [CTRL_WRITE, REG_DEFVALA, 0x00],
    [CTRL_WRITE, REG_DEFVALB, 0x00],
    [CTRL_WRITE, REG_INTCONA, 0x00],
    [CTRL_WRITE, REG_INTCONB, 0x00],
    [CTRL_WRITE, REG_GPPUA, 0x00],
    [CTRL_WRITE, REG_GPPUB, 0x00],
    [CTRL_WRITE, REG_OLATA, 0xFF],
    [CTRL_WRITE, REG_OLATB, 0x00]
];

pub struct KeypadIo<SPI: SpiDevice, INT: Wait> {
    spi: SPI,
    interrupt: INT,
}

impl<SPI: SpiDevice, INT: Wait> KeypadIo<SPI, INT> {
    pub fn new(spi: SPI, interrupt: INT) -> Self {
        Self { spi, interrupt }
    }

    pub async fn setup(&mut self) -> Result<(), SPI::Error> {
        for cmd in STARTUP_SEQUENCE {
            self.spi.write(cmd).await?;
        }
        Ok(())
    }

    pub async fn wait_for_int(&mut self) -> Result<(), SPI::Error> {
        self.spi.write(&[CTRL_WRITE, REG_OLATA, 0xFF]).await?;
        self.interrupt.wait_for_high().await.unwrap();
        Ok(())
    }

    pub async fn poll_all(&mut self) -> Result<u64, SPI::Error> {
        let mut result: u64 = 0;
        let mut col: u8 = 0b1000000;

        while col > 0 {
            result = result << 7;
            self.spi.write(&[CTRL_WRITE, REG_OLATA, col]).await?;
            Timer::after_micros(10).await;
            let mut read = [0; 3];
            self.spi
                .transfer(&mut read, &[CTRL_READ, REG_GPIOB, 0])
                .await?;
            result = result | ((read[2] as u64) & 0x7F);
            col = col >> 1;
        }

        Ok(result)
    }
}
