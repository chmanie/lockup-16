use portable_atomic::Ordering;

use crate::{app::App, ATOMIC_VALUES};
pub const CHANNELS: usize = 1;

pub async fn run(app: App<CHANNELS>) {
    let _ = app.make_foo().await;
    let fut1 = async {
        loop {
            let _ = ATOMIC_VALUES[app.channels[0]].load(Ordering::Relaxed);
            app.delay_millis(10).await;
            app.midi_send_cc(0, 10).await;
        }
    };

    fut1.await;
}
