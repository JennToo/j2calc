#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join;
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    peripherals,
    spi::{Config, Phase, Polarity, Spi},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex, signal::Signal};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use static_cell::StaticCell;

mod lcd;

const VCOM_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

type Display = lcd::Lcd<Spi<'static, peripherals::SPI0, embassy_rp::spi::Async>, Output<'static>>;
static DISPLAY_MEMORY: StaticCell<Display> = StaticCell::new();
type FramebufferMutex = Mutex<NoopRawMutex, lcd::BinaryFramebuffer>;
static FB_MEMORY: StaticCell<FramebufferMutex> = StaticCell::new();
static REDRAW_MEMORY: StaticCell<Signal<NoopRawMutex, ()>> = StaticCell::new();

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
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_7, Level::Low);

    let mut lcd_spi_config = Config::default();
    lcd_spi_config.frequency = 2_000_000;
    lcd_spi_config.phase = Phase::CaptureOnSecondTransition;
    lcd_spi_config.polarity = Polarity::IdleLow;
    let lcd_spi = Spi::new_txonly(p.SPI0, p.PIN_22, p.PIN_23, p.DMA_CH0, Irqs, lcd_spi_config);
    let lcd = DISPLAY_MEMORY.init(lcd::Lcd::new(lcd_spi, Output::new(p.PIN_25, Level::Low)));
    let fb = FB_MEMORY.init(Mutex::new(lcd::BinaryFramebuffer::new()));
    let redraw = REDRAW_MEMORY.init(Signal::new());

    spawner.spawn(display_update(lcd, fb, redraw).unwrap());

    let mut fb = fb.lock().await;
    lcd::draw_splash(&mut *fb);
    redraw.signal(());
}

#[embassy_executor::task]
async fn display_update(
    display: &'static mut Display,
    framebuffer: &'static FramebufferMutex,
    redraw: &'static Signal<NoopRawMutex, ()>,
) {
    display.clear_screen().await.unwrap();

    loop {
        if redraw.signaled() {
            let fb = framebuffer.lock().await;
            display.draw_framebuffer(&fb).await.unwrap();
            redraw.reset();
        } else {
            display.update_vcom().await.unwrap();
        }

        let timer = Timer::after(VCOM_REFRESH_INTERVAL);
        join::join(timer, redraw.wait()).await;
    }
}
