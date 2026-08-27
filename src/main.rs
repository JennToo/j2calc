#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, gpio, peripherals,
    spi::{Config, Phase, Polarity, Spi},
};
use embassy_time::Timer;
use gpio::{Level, Output};
use panic_probe as _;

mod lcd;

// Program metadata for `picotool info`.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"j2calc"),
    embassy_rp::binary_info::rp_program_description!(c"Programmer's calculator"),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<peripherals::DMA_CH0>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_7, Level::Low);

    let mut lcd_spi_config = Config::default();
    lcd_spi_config.frequency = 1_000_000;
    lcd_spi_config.phase = Phase::CaptureOnSecondTransition;
    lcd_spi_config.polarity = Polarity::IdleLow;
    let lcd_spi = Spi::new_txonly(p.SPI0, p.PIN_22, p.PIN_23, p.DMA_CH0, Irqs, lcd_spi_config);
    let mut lcd = lcd::Lcd::new(lcd_spi, Output::new(p.PIN_25, Level::Low));

    lcd.clear_screen().await.unwrap();
    lcd::draw_splash(&mut lcd);
    lcd.flush().await.unwrap();

    loop {
        info!("led on!");
        led.set_high();
        Timer::after_millis(500).await;

        info!("led off!");
        led.set_low();
        Timer::after_millis(500).await;

        // TODO: Move to separate task
        lcd.update_vcom().await.unwrap();
    }
}
