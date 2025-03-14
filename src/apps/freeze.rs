use defmt::info;
use embassy_time::{with_timeout, Duration};
use portable_atomic::Ordering;

use crate::{app::App, MAX_VALUES_ADC};
pub const CHANNELS: usize = 1;

pub async fn run(app: App<CHANNELS>) {
    let _ = app.make_in_jack(0).await;
    let fut1 = async {
        loop {
            let _ = MAX_VALUES_ADC[app.channels[0]].load(Ordering::Relaxed);
            app.delay_millis(10).await;
            if with_timeout(Duration::from_millis(1000), app.midi_send_cc(0, 10))
                .await
                .is_err()
            {
                info!("ERR");
                continue;
            }
        }
    };

    fut1.await;
}
