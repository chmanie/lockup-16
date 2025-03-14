use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_rp::peripherals::USB;
use embassy_rp::peripherals::{UART0, UART1};
use embassy_rp::uart::{Async, Uart, UartTx};
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcAcmState};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::class::web_usb::{Config as WebUsbConfig, State as WebUsbState, Url, WebUsb};
use embassy_usb::driver::Driver;
use embassy_usb::{Builder, Config as UsbConfig};

use crate::CHAN_X_RX;

pub struct WebEndpoints<'d, D: Driver<'d>> {
    write_ep: D::EndpointIn,
    read_ep: D::EndpointOut,
}

impl<'d, D: Driver<'d>> WebEndpoints<'d, D> {
    fn new(builder: &mut Builder<'d, D>, config: &'d WebUsbConfig<'d>) -> Self {
        let mut func = builder.function(0xff, 0x00, 0x00);
        let mut iface = func.interface();
        let mut alt = iface.alt_setting(0xff, 0x00, 0x00, None);

        let write_ep = alt.endpoint_bulk_in(config.max_packet_size);
        let read_ep = alt.endpoint_bulk_out(config.max_packet_size);

        WebEndpoints { write_ep, read_ep }
    }
}

pub async fn start_transports(
    spawner: &Spawner,
    usb_driver: usb::Driver<'static, USB>,
    uart0: UartTx<'static, UART0, Async>,
    uart1: Uart<'static, UART1, Async>,
) {
    spawner
        .spawn(run_transports(usb_driver, uart0, uart1))
        .unwrap();
}

pub async fn msg_receive_loop() {
    let midi_tx = async {
        loop {
            let _msg = CHAN_X_RX.receive().await;
            info!("FREE: {}", CHAN_X_RX.free_capacity());
        }
    };

    midi_tx.await;
}

#[embassy_executor::task]
async fn run_transports(
    usb_driver: usb::Driver<'static, USB>,
    uart0: UartTx<'static, UART0, Async>,
    uart1: Uart<'static, UART1, Async>,
) {
    let mut usb_config = UsbConfig::new(0xf569, 0x1);
    usb_config.manufacturer = Some("ACME");
    usb_config.product = Some("Device");
    usb_config.serial_number = Some("12345678");
    // 0x0 (Major) | 0x1 (Minor) | 0x0 (Patch)
    usb_config.device_release = 0x010;
    usb_config.max_power = 500;
    usb_config.max_packet_size_0 = 64;
    usb_config.device_class = 0xEF;
    usb_config.device_sub_class = 0x02;
    usb_config.device_protocol = 0x01;
    usb_config.composite_with_iads = true;

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];

    let webusb_config = WebUsbConfig {
        max_packet_size: 64,
        vendor_code: 1,
        landing_url: Some(Url::new("http://localhost:8080")),
    };

    let mut webusb_state = WebUsbState::new();
    let mut logger_state = CdcAcmState::new();

    let mut usb_builder = Builder::new(
        usb_driver,
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );

    WebUsb::configure(&mut usb_builder, &mut webusb_state, &webusb_config);
    let webusb = WebEndpoints::new(&mut usb_builder, &webusb_config);

    let usb_midi = MidiClass::new(&mut usb_builder, 1, 1, 64);

    let usb_logger = CdcAcmClass::new(&mut usb_builder, &mut logger_state, 64);
    let log_fut = embassy_usb_logger::with_class!(1024, log::LevelFilter::Info, usb_logger);

    let mut usb = usb_builder.build();

    let msg_fut = msg_receive_loop();
    join3(usb.run(), log_fut, msg_fut).await;
}
