use core::array;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use embassy_time::Timer;
use midi2::{
    channel_voice1::{ChannelVoice1, ControlChange},
    ux::u4,
    Channeled,
};
use portable_atomic::Ordering;

use crate::{utils::u16_to_u7, XRxMsg, ATOMIC_VALUES};

pub struct Foo {
    channel: usize,
}

impl Foo {
    pub fn get_value(&self) -> u16 {
        ATOMIC_VALUES[self.channel].load(Ordering::Relaxed)
    }
}

pub struct App<const N: usize> {
    app_id: usize,
    pub channels: [usize; N],
    sender: Sender<'static, NoopRawMutex, XRxMsg, 128>,
}

impl<const N: usize> App<N> {
    pub fn new(
        app_id: usize,
        start_channel: usize,
        sender: Sender<'static, NoopRawMutex, XRxMsg, 128>,
    ) -> Self {
        let channels: [usize; N] = array::from_fn(|i| start_channel + i);
        Self {
            app_id,
            channels,
            sender,
        }
    }

    pub async fn make_foo(&self) -> Foo {
        Foo {
            channel: self.channels[0],
        }
    }

    pub async fn delay_millis(&self, millis: u64) {
        Timer::after_millis(millis).await
    }

    pub async fn midi_send_cc(&self, chan: usize, val: u16) {
        let midi_channel = u4::new(1);
        let mut cc = ControlChange::<[u8; 3]>::new();
        cc.set_control(u16_to_u7(102 + chan as u16));
        cc.set_control_data(u16_to_u7(val));
        cc.set_channel(midi_channel);
        let msg = ChannelVoice1::ControlChange(cc);
        self.send_midi_msg(msg).await;
    }

    pub async fn send_midi_msg(&self, msg: ChannelVoice1<[u8; 3]>) {
        self.sender.send(XRxMsg::Default).await;
    }
}
