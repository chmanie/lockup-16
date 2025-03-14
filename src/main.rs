#![no_std]
#![no_main]

#[macro_use]
mod macros;

mod app;
mod apps;
mod transport;
mod utils;

use core::sync::atomic::AtomicU16;

use defmt::info;
use embassy_executor::{Executor, Spawner};
use embassy_rp::bind_interrupts;
use embassy_rp::block::ImageDef;
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::{UART0, UART1, USB};
use embassy_rp::uart::{self, Async as UartAsync, Config as UartConfig, Uart, UartTx};
use embassy_rp::usb;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::Timer;
use midi2::channel_voice1::ChannelVoice1;

use {defmt_rtt as _, panic_probe as _};

use static_cell::StaticCell;

use apps::run_app_by_id;

#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: ImageDef = ImageDef::secure_exe();

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    UART0_IRQ => uart::InterruptHandler<UART0>;
    UART1_IRQ => uart::InterruptHandler<UART1>;
});

/// Messages from core 1 to core 0
#[derive(Clone)]
pub enum XRxMsg {
    MidiMessage(ChannelVoice1<[u8; 3]>),
}

static EXECUTOR1: StaticCell<Executor> = StaticCell::new();
static mut CORE1_STACK: Stack<131_072> = Stack::new();

pub static MAX_VALUES_ADC: [AtomicU16; 16] = [const { AtomicU16::new(0) }; 16];
static CHAN_X_1: StaticCell<Channel<NoopRawMutex, (usize, XRxMsg), 128>> = StaticCell::new();
pub static CHAN_X_RX: Channel<CriticalSectionRawMutex, (usize, XRxMsg), 64> = Channel::new();

#[embassy_executor::task(pool_size = 16)]
async fn run_app(
    number: usize,
    start_channel: usize,
    sender: Sender<'static, NoopRawMutex, (usize, XRxMsg), 128>,
) {
    // This is a requirement
    Timer::after_millis(1000).await;
    run_app_by_id(number, start_channel, sender).await;
}

#[embassy_executor::task]
async fn x_rx(receiver: Receiver<'static, NoopRawMutex, (usize, XRxMsg), 128>) {
    loop {
        let msg = receiver.receive().await;
        CHAN_X_RX.send(msg).await;
    }
}

#[embassy_executor::task]
async fn control() {
    loop {
        info!("I'm alive!");
        Timer::after_secs(2).await;
    }
}

#[embassy_executor::task]
async fn main_core1(spawner: Spawner) {
    let chan_x_1 = CHAN_X_1.init(Channel::new());
    spawner.spawn(x_rx(chan_x_1.receiver())).unwrap();

    let apps: [usize; 16] = [1; 16];
    for (i, app_id) in apps.iter().enumerate() {
        spawner
            .spawn(run_app(*app_id, i, chan_x_1.sender()))
            .unwrap();
    }
    // spawner.spawn(control()).unwrap();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // MIDI
    let mut uart_config = UartConfig::default();
    // Classic MIDI baud rate
    uart_config.baudrate = 31250;
    // MIDI Thru
    let uart0: UartTx<'_, _, UartAsync> = UartTx::new(p.UART0, p.PIN_0, p.DMA_CH2, uart_config);
    // MIDI In/Out
    let uart1 = Uart::new(
        p.UART1,
        p.PIN_8,
        p.PIN_9,
        Irqs,
        p.DMA_CH3,
        p.DMA_CH4,
        uart_config,
    );

    let usb_driver = usb::Driver::new(p.USB, Irqs);

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| {
                spawner.spawn(main_core1(spawner)).unwrap();
            });
        },
    );

    // spawner.spawn(main_core1(spawner)).unwrap();
    // spawner.spawn(control()).unwrap();

    transport::start_transports(&spawner, usb_driver, uart0, uart1).await;
}
