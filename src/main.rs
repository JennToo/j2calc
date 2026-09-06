#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts, gpio, peripherals,
    spi::{Config, Phase, Polarity, Spi},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use gpio::{Level, Output};
use panic_probe as _;
use static_cell::StaticCell;

mod lcd;

const DISPLAY_REFRESH_PERIOD: Duration = Duration::from_hz(10);

type Display = lcd::Lcd<Spi<'static, peripherals::SPI0, embassy_rp::spi::Async>, Output<'static>>;
type DisplayMutex = Mutex<NoopRawMutex, Display>;
static DISPLAY_MEMORY: StaticCell<DisplayMutex> = StaticCell::new();

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

    info!(
        "{} took {} microseconds",
        name,
        end.duration_since(start).as_micros()
    );
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_7, Level::Low);

    let mut lcd_spi_config = Config::default();
    lcd_spi_config.frequency = 2_000_000;
    lcd_spi_config.phase = Phase::CaptureOnSecondTransition;
    lcd_spi_config.polarity = Polarity::IdleLow;
    let lcd_spi = Spi::new_txonly(p.SPI0, p.PIN_22, p.PIN_23, p.DMA_CH0, Irqs, lcd_spi_config);
    let display = DISPLAY_MEMORY.init(Mutex::new(lcd::Lcd::new(
        lcd_spi,
        Output::new(p.PIN_25, Level::Low),
    )));

    spawner.spawn(display_update(display).unwrap());

    let mut y_offset = 240;

    loop {
        let now = Instant::now();
        let end_of_frame = now + DISPLAY_REFRESH_PERIOD;

        y_offset -= 1;
        if y_offset < -240 * 6 {
            y_offset = 0;
        }
        {
            let mut lcd = display.lock().await;
            lcd::draw_splash(&mut *lcd, y_offset).unwrap();
        }

        Timer::at(end_of_frame).await;
    }
}

#[embassy_executor::task]
async fn display_update(display: &'static DisplayMutex) {
    {
        let mut lcd = display.lock().await;
        lcd.clear_screen().await.unwrap();
        lcd.flush().await.unwrap();
    }

    loop {
        let now = Instant::now();
        let end_of_frame = now + DISPLAY_REFRESH_PERIOD;

        {
            let mut lcd = display.lock().await;
            lcd.flush().await.unwrap();
        }

        let now = Instant::now();
        if now > end_of_frame {
            warn!(
                "Display update missed timing by {} us",
                (now - end_of_frame).as_micros()
            );
        }

        Timer::at(end_of_frame).await;
    }
}
