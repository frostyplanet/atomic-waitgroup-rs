use captains_log::*;

pub fn _setup_log() {
    #[cfg(feature = "trace_log")]
    {
        let format = recipe::LOG_FORMAT_THREADED_DEBUG;
        #[cfg(miri)]
        {
            let _ = std::fs::remove_file("/tmp/wg_miri.log");
            let file = LogRawFile::new("/tmp", "wg_miri.log", Level::Trace, format);
            captains_log::Builder::default()
                .tracing_global()
                .add_sink(file)
                .test()
                .build()
                .expect("log setup");
        }
        #[cfg(not(miri))]
        {
            let ring = ringfile::LogRingFile::new(
                "/tmp/wg_ring.log",
                500 * 1024 * 1024,
                Level::Trace,
                format,
            );
            let mut config = Builder::default()
                .signal(signal_consts::SIGINT)
                .signal(signal_consts::SIGTERM)
                .tracing_global()
                .add_sink(ring)
                .add_sink(LogConsole::new(
                    ConsoleTarget::Stdout,
                    Level::Info,
                    recipe::LOG_FORMAT_DEBUG,
                ));
            config.dynamic = true;
            config.build().expect("log_setup");
        }
    }
    #[cfg(not(feature = "trace_log"))]
    {
        let _ = recipe::env_logger("LOG_FILE", "LOG_LEVEL").build().expect("log setup");
    }
}

#[macro_export]
macro_rules! runtime_block_on {
    ($threads: expr, $f: expr) => {{
        #[cfg(feature = "smol")]
        {
            log::info!("run with smol");
            smol::block_on($f)
        }
        #[cfg(not(feature = "smol"))]
        {
            let runtime_flag = std::env::var("SINGLE_THREAD_RUNTIME").unwrap_or("".to_string());
            let rt = if runtime_flag.len() > 0 {
                log::info!("run with tokio current thread");
                tokio::runtime::Builder::new_current_thread()
                .enable_all().build().unwrap()
            } else {
                log::info!("run with tokio multi thread");
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads($threads).build().unwrap()
            };
            rt.block_on($f)
        }
    }};
}

#[macro_export]
macro_rules! async_spawn_detach {
    ($f: expr) => {{
        #[cfg(feature = "smol")]
        {
            let _ = smol::spawn($f).detach();
        }
        #[cfg(not(feature = "smol"))]
        {
            let _ = tokio::spawn($f);
        }
    }};
}

#[macro_export]
#[allow(dead_code)]
macro_rules! async_join_result {
    ($th: expr) => {{
        #[cfg(feature = "smol")]
        {
            $th.await
        }
        #[cfg(not(feature = "smol"))]
        {
            $th.await.expect("join")
        }
    }};
}


#[allow(dead_code)]
pub async fn sleep(duration: std::time::Duration) {
    #[cfg(feature = "smol")]
    {
        smol::Timer::after(duration).await;
    }
    #[cfg(not(feature = "smol"))]
    {
        tokio::time::sleep(duration).await;
    }
}
