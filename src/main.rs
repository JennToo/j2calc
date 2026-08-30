#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, gpio, peripherals,
    spi::{Config, Phase, Polarity, Spi},
};
use embassy_time::{Duration, Instant, Timer};
use gpio::{Level, Output};
use panic_probe as _;

mod lcd;

const DISPLAY_REFRESH_PERIOD: Duration = Duration::from_hz(8);

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

async fn measure_duration(name: &str, f: impl AsyncFnOnce() -> ()) {
    let start = embassy_time::Instant::now();
    f().await;
    let end = embassy_time::Instant::now();

    info!("{} took {} microseconds", name, end.duration_since(start).as_micros());
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_7, Level::Low);

    let mut lcd_spi_config = Config::default();
    lcd_spi_config.frequency = 2_000_000;
    lcd_spi_config.phase = Phase::CaptureOnSecondTransition;
    lcd_spi_config.polarity = Polarity::IdleLow;
    let lcd_spi = Spi::new_txonly(p.SPI0, p.PIN_22, p.PIN_23, p.DMA_CH0, Irqs, lcd_spi_config);
    let mut lcd = lcd::Lcd::new(lcd_spi, Output::new(p.PIN_25, Level::Low));

    lcd.clear_screen().await.unwrap();
    lcd.flush().await.unwrap();

    let mut y_offset = 240;
    let mut next_led_toggle = Instant::now();

    loop {
        let now = Instant::now();
        let end_of_frame = now + DISPLAY_REFRESH_PERIOD;

        if now > next_led_toggle {
            led.set_low();
        }

        y_offset -= 2;
        if y_offset < -240 * 6 {
            y_offset = 0;
        }
        lcd::draw_splash(&mut lcd, y_offset).unwrap();

        lcd.flush().await.unwrap();

        let now = Instant::now();
        if now > end_of_frame {
            warn!("Missed timing by {} us", (now - end_of_frame).as_micros());
            led.set_high();
            next_led_toggle = now + Duration::from_secs(1);
        }

        Timer::at(end_of_frame).await;
    }
}

#[embassy_executor::task]
async fn screen_updater() {
    
}
