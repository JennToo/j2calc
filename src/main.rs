#![no_std]
#![no_main]

use core::fmt::Write;
use defmt::info;
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_rp::{
    bind_interrupts,
    gpio::{Input, Level, Output, Pull},
    peripherals,
    spi::{Config, Phase, Polarity, Spi},
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex, signal::Signal};
use embassy_time::{Duration, Timer};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::BinaryColor};
use panic_probe as _;
use static_cell::StaticCell;

mod keypad;
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
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<peripherals::DMA_CH0>,
                 embassy_rp::dma::InterruptHandler<peripherals::DMA_CH1>,
                 embassy_rp::dma::InterruptHandler<peripherals::DMA_CH2>;
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

    let mut keypad_spi_config = Config::default();
    keypad_spi_config.frequency = 1_000_000; // Can go as high as 10 MHz
    keypad_spi_config.phase = Phase::CaptureOnSecondTransition;
    keypad_spi_config.polarity = Polarity::IdleLow;
    let keypad_spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(Spi::new(
        p.SPI1,
        p.PIN_10,
        p.PIN_11,
        p.PIN_24,
        p.DMA_CH1,
        p.DMA_CH2,
        Irqs,
        keypad_spi_config,
    ));
    let keypad_spi_device = SpiDevice::new(&keypad_spi_bus, Output::new(p.PIN_5, Level::High));
    let mut keypad = keypad::KeypadIo::new(keypad_spi_device, Input::new(p.PIN_6, Pull::Down));

    let mut text_fb = lcd::TextFramebuffer::new();

    spawner.spawn(display_update(lcd, fb, redraw).unwrap());

    {
        let mut fb = fb.lock().await;
        fb.clear(BinaryColor::Off).unwrap();
    }
    redraw.signal(());

    keypad.setup().await.unwrap();
    loop {
        Timer::after_millis(10).await; // Debounce
        let val = keypad.poll_all().await.unwrap();
        info!("Button Mask {:049b}", val);
        Timer::after(Duration::from_millis(1)).await;

        text_fb.clear();
        write!(text_fb, "Buttons {:049b}", val).unwrap();
        {
            let mut fb = fb.lock().await;
            fb.clear(BinaryColor::Off).unwrap();
            text_fb.draw(&mut *fb);
        }
        redraw.signal(());

        if val == 0 {
            led.set_low();
            keypad.wait_for_int().await.unwrap();
        } else {
            led.set_high();
        }
    }
}

#[embassy_executor::task]
async fn display_update(
    display: &'static mut Display,
    framebuffer: &'static FramebufferMutex,
    redraw: &'static Signal<NoopRawMutex, ()>,
) {
    display.clear_screen().await.unwrap();

    loop {
        let timer = Timer::after(VCOM_REFRESH_INTERVAL);

        match select(redraw.wait(), timer).await {
            Either::First(_) => {
                redraw.reset();
                let fb = framebuffer.lock().await;
                display.draw_framebuffer(&fb).await.unwrap();
            }
            Either::Second(_) => {
                display.update_vcom().await.unwrap();
            }
        }
    }
}
