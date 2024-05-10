use atomic_waitgroup::WaitGroup;
use rand::{rngs::OsRng, RngCore};
use std::time::Duration;
use tokio::time::sleep;

fn make_runtime(threads: usize) -> tokio::runtime::Runtime {
    return tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(threads)
        .build()
        .unwrap();
}

#[test]
fn test_wait_group_wait0() {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    make_runtime(10).block_on(async move {
        for _i in 0..threads {
            let _wg = wg.clone();
            wg.add(1);
            tokio::spawn(async move {
                sleep(Duration::from_secs(1)).await;
                _wg.done();
            });
        }
        wg.wait().await;
    })
}

#[test]
fn test_wait_group_waitto3() {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    make_runtime(8).block_on(async move {
        for _i in 0..threads {
            let _wg = wg.clone();
            wg.add(1);
            tokio::spawn(async move {
                sleep(Duration::from_secs(_i)).await;
                _wg.done();
            });
        }
        wg.wait_to(3).await;
        let left = wg.left();
        assert!(left <= 3, "{}", left);
        println!("left {}", left);
    })
}

#[test]
fn test_wait_group_multi_waitto_and_add() {
    let wg = WaitGroup::new();
    make_runtime(8).block_on(async move {
        for _i in 0u64..1000 {
            wg.add(1);
            let _wg = wg.clone();
            tokio::spawn(async move {
                sleep(Duration::from_millis((_i % 2) + 1)).await;
                _wg.done();
            });
            wg.wait_to(10).await;
        }
        wg.wait().await;
        println!("done");
    });
}

#[test]
fn test_wait_group_loop() {
    let wg = WaitGroup::new();
    make_runtime(2).block_on(async move {
        let mut loop_cnt = 0;
        for _ in 0..1000 {
            let threads = OsRng.next_u32() % 10 + 1;
            loop_cnt += 1;
            println!("loop_cnt={} threads={}", loop_cnt, threads);

            for _i in 0..threads {
                wg.add(1);
                let _wg = wg.clone();
                std::thread::spawn(move || {
                    let millis = (OsRng.next_u32() % 10) as u64;
                    std::thread::sleep(Duration::from_millis(millis));
                    _wg.done();
                });
            }
            let millis = (OsRng.next_u32() % 10) as u64;
            tokio::time::sleep(Duration::from_millis(millis)).await;
            wg.wait().await;
        }
    });
}

#[test]
fn test_guard() {
    let wg = WaitGroup::new();
    let threads = 10;
    assert_eq!(wg.left(), 0);
    wg.add(1);
    assert_eq!(wg.left(), 1);
    wg.done();
    assert_eq!(wg.left(), 0);
    make_runtime(8).block_on(async move {
        for _i in 0..threads {
            let _guard = wg.add_guard();
            tokio::spawn(async move {
                sleep(Duration::from_secs(_i)).await;
                drop(_guard);
            });
        }
        wg.wait_to(3).await;
        let left = wg.left();
        assert!(left <= 3, "{}", left);
        println!("left {}", left);
    })
}

#[test]
#[should_panic]
fn test_multiple_wait_panic() {
    let wg = WaitGroup::new();
    make_runtime(1).block_on(async move {
        wg.add(1);
        let _wg = wg.clone();
        tokio::spawn(async move {
            _wg.wait().await;
        });
        sleep(Duration::from_secs(1)).await;
        // This expect to panic, NOTE that "should_panic" do not worker in spawned coroutines.
        wg.wait().await;
    });
}

#[test]
#[should_panic]
fn test_done_overflow() {
    let wg = WaitGroup::new();
    wg.add(1);
    wg.done_many(2);
}
