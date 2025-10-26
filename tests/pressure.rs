use atomic_waitgroup::WaitGroup;
use rand::{rngs::OsRng, RngCore};
use std::time::Duration;
use captains_log::{logfn, *};
use rstest::*;

mod common;
use common::*;

#[fixture]
fn setup_log() {
    _setup_log();
}

#[logfn]
#[rstest]
fn pressure_wait_group_loop(setup_log: ()) {
    let wg = WaitGroup::new();
    runtime_block_on!(1, async move {
        let mut loop_cnt = 0;
        for _ in 0..1000 {
            let threads = OsRng.next_u32() % 10 + 1;
            loop_cnt += 1;
            info!("loop_cnt={} threads={}", loop_cnt, threads);

            for _i in 0..threads {
                wg.add(1);
                let _wg = wg.clone();
                std::thread::spawn(move || {
                    let millis = (OsRng.next_u32() % 10) as u64;
                    std::thread::sleep(Duration::from_millis(millis));
                    _wg.done();
                });
            }
            #[cfg(not(miri))]
            {
                let millis = (OsRng.next_u32() % 10) as u64;
                sleep(Duration::from_millis(millis)).await;
            }
            wg.wait().await;
        }
    });
}

