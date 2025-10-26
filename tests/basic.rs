use atomic_waitgroup::WaitGroup;
use std::time::Duration;
use captains_log::logfn;
use rstest::*;

mod common;
use common::*;

#[fixture]
fn setup_log() {
    _setup_log();
}

#[logfn]
#[rstest]
fn basic_wait_group_wait0(setup_log: ()) {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    runtime_block_on!(10, async move {
        for _i in 0..threads {
            let _wg = wg.clone();
            wg.add(1);
            async_spawn_detach!(async move {
                sleep(Duration::from_secs(1)).await;
                _wg.done();
            });
        }
        wg.wait().await;
    });
}

#[logfn]
#[rstest]
fn basic_wait_group_waitto3(setup_log: ()) {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    runtime_block_on!(8, async move {
        for _i in 0..threads {
            let _wg = wg.clone();
            wg.add(1);
            async_spawn_detach!(async move {
                sleep(Duration::from_secs(_i)).await;
                _wg.done();
            });
        }
        wg.wait_to(3).await;
        let left = wg.left();
        assert!(left <= 3, "{}", left);
        println!("left {}", left);
    });
}

#[logfn]
#[rstest]
fn basic_wait_group_multi_waitto_and_add(setup_log: ()) {
    let wg = WaitGroup::new();
    runtime_block_on!(8, async move {
        for _i in 0u64..1000 {
            wg.add(1);
            let _wg = wg.clone();
            async_spawn_detach!(async move {
                sleep(Duration::from_millis((_i % 2) + 1)).await;
                _wg.done();
            });
            wg.wait_to(10).await;
        }
        wg.wait().await;
        println!("done");
    });
}

#[logfn]
#[rstest]
fn basic_guard(setup_log: ()) {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    runtime_block_on!(8, async move {
        for _i in 0..threads {
            let _guard = wg.add_guard();
            async_spawn_detach!(async move {
                sleep(Duration::from_secs(_i)).await;
                drop(_guard);
            });
        }
        wg.wait_to(3).await;
        let left = wg.left();
        assert!(left <= 3, "{}", left);
        println!("left {}", left);
    });
}

#[cfg(not(feature="trace_log"))]
#[logfn]
#[test]
#[should_panic]
#[cfg_attr(miri, ignore)]
fn basic_multiple_wait_panic() {
    let wg = WaitGroup::new();
    runtime_block_on!(1, async move {
        wg.add(1);
        let _wg = wg.clone();
        async_spawn_detach!(async move {
            _wg.wait().await;
        });
        sleep(Duration::from_secs(1)).await;
        // This expect to panic, NOTE that "should_panic" do not worker in spawned coroutines.
        wg.wait().await;
    });
}

#[cfg(not(feature="trace_log"))]
#[logfn]
#[test]
#[should_panic]
#[cfg_attr(miri, ignore)]
fn basic_done_overflow() {
    let wg = WaitGroup::new();
    wg.add(1);
    wg.done_many(2);
}
