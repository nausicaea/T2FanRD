#![warn(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::module_name_repetitions
)]

use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use arraydeque::ArrayDeque;
use fan_controller::{Fan, FanController};
use nonempty::NonEmpty as NonEmptyVec;
use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::flag as signal_flag;

use config::load_fan_configs;
use error::{Error, Result};

mod config;
mod error;
mod fan_controller;

#[cfg(not(any(target_os = "linux", debug_assertions)))]
compile_error!("This tool is only developed for Linux systems.");

const LOCK_FILE: &str = "/run/t2fanrd.lock";

fn acquire_lock_file<P: AsRef<Path>>(lock_file: P) -> Result<File> {
    #[allow(clippy::suspicious_open_options)]
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(lock_file)
        .map_err(Error::LockWrite)?;

    // SAFETY: valid fd, correct flock constants
    let ret = unsafe {
        use std::os::unix::io::AsRawFd;
        libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB)
    };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            return Err(Error::AlreadyRunning);
        }
        return Err(Error::LockWrite(err));
    }

    Ok(file) // keep File alive — lock released when dropped
}

fn get_current_euid() -> libc::uid_t {
    // SAFETY: FFI call with no preconditions
    unsafe { libc::geteuid() }
}

fn find_fans() -> Result<NonEmptyVec<Fan>> {
    // /sys/class/hwmon/hwmon*/device/name == "applesmc"
    // /sys/class/hwmon/hwmon*/device/fan*
    let mut fans = Vec::default();
    for path in glob::glob("/sys/class/hwmon/hwmon*/device/name")? {
        let path = path?;
        let mut device_name = String::default();
        File::open(&path)
            .and_then(|mut f| f.read_to_string(&mut device_name))
            .map_err(Error::FanSearch)?;
        if device_name.trim() != "applesmc" {
            continue;
        }

        let device_path = path.parent().ok_or(Error::NoFan)?;
        for fan_input in glob::glob(&format!("{}/fan*_input", device_path.display()))? {
            let mut fan_input = fan_input?;
            let fan_name = fan_input
                .file_name()
                .and_then(|f| f.to_str())
                .and_then(|f| f.strip_suffix("_input"))
                .ok_or(Error::NoFan)?;
            #[allow(clippy::unnecessary_to_owned)]
            fan_input.set_file_name(fan_name.to_string());
            fans.push(Fan::new(fan_input));
        }
    }

    NonEmptyVec::from_vec(fans).ok_or(Error::NoFan)
}

fn read_temp_file(temp_file: &mut File, temp_buf: &mut String) -> Result<u8> {
    temp_file
        .read_to_string(temp_buf)
        .map_err(Error::TempRead)?;

    temp_file.rewind().map_err(Error::TempSeek)?;

    let temp = temp_buf.trim_end().parse::<u32>().map_err(Error::TempParse);
    temp_buf.clear();
    temp.map(|t| (t / 1000) as u8)
}

fn find_temp_file(temps: glob::Paths, temp_buf: &mut String) -> Option<File> {
    for temp_path_res in temps {
        let Ok(temp_path) = temp_path_res else {
            log::error!("Unable to read glob path");
            continue;
        };

        let Ok(mut temp_file) = File::open(temp_path) else {
            log::error!("Unable to open temperature sensor");
            continue;
        };

        if read_temp_file(&mut temp_file, temp_buf).is_ok() {
            return Some(temp_file);
        }
    }

    None
}

fn find_cpu_temp_file(temp_buf: &mut String) -> Result<File> {
    let temps = glob::glob("/sys/devices/platform/coretemp.0/hwmon/hwmon*/temp1_input")?;
    find_temp_file(temps, temp_buf).ok_or(Error::NoCpu)
}

fn find_gpu_temp_file(temp_buf: &mut String) -> Result<Option<File>> {
    let temps = glob::glob("/sys/class/drm/card0/device/hwmon/hwmon*/temp1_input")?;
    Ok(find_temp_file(temps, temp_buf))
}

fn main() -> ExitCode {
    env_logger::init();

    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn start_temp_loop(
    mut temp_buffer: String,
    mut cpu_temp_file: File,
    mut gpu_temp_file: Option<File>,
    fans: &mut NonEmptyVec<FanController>,
) -> Result<()> {
    let term = Arc::new(AtomicBool::new(false));
    signal_flag::register(SIGINT, term.clone()).map_err(Error::Signal)?;
    signal_flag::register(SIGTERM, term.clone()).map_err(Error::Signal)?;
    signal_flag::register(SIGQUIT, term.clone()).map_err(Error::Signal)?;
    signal_flag::register(SIGHUP, term.clone()).map_err(Error::Signal)?;

    let mut last_temp = 0;
    let mut temps = ArrayDeque::<u8, 50, arraydeque::Wrapping>::new();
    let mut was_long_sleep = false;
    while !term.load(Ordering::Relaxed) {
        let cpu_temp = read_temp_file(&mut cpu_temp_file, &mut temp_buffer)?;
        let temp = if let Some(gpu_temp_file) = &mut gpu_temp_file {
            let gpu_temp = read_temp_file(gpu_temp_file, &mut temp_buffer)?;
            if gpu_temp > cpu_temp {
                gpu_temp
            } else {
                cpu_temp
            }
        } else {
            cpu_temp
        };

        temps.push_back(temp);
        if was_long_sleep {
            // Avoid messing up the mean due to the longer sleep.
            for _ in 0..9 {
                temps.push_back(temp);
            }
        }

        let sum_temp: u32 = temps.iter().map(|t| *t as u32).sum();
        let mean_temp: u8 = u8::try_from(sum_temp / (temps.len() as u32))
            .map_err(Error::TempMean)?;
        if mean_temp == last_temp {
            std::thread::sleep(std::time::Duration::from_secs(1));
            was_long_sleep = true;
        } else {
            last_temp = mean_temp;
            for fan in fans.iter_mut() {
                fan.set_speed(fan.calc_speed(mean_temp))?;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
            was_long_sleep = false;
        }
    }

    Ok(())
}

fn real_main() -> Result<()> {
    if get_current_euid() != 0 {
        return Err(Error::NotRoot);
    }

    let lock = acquire_lock_file(LOCK_FILE)?;

    let mut temp_buffer = String::new();

    let fans = find_fans()?;
    let mut fan_controllers = load_fan_configs(fans)?;
    let cpu_temp_file = find_cpu_temp_file(&mut temp_buffer)?;
    let gpu_temp_file = find_gpu_temp_file(&mut temp_buffer)?;

    for fan in fan_controllers.iter_mut() {
        fan.set_manual(true)?;
    }

    let res = start_temp_loop(
        temp_buffer,
        cpu_temp_file,
        gpu_temp_file,
        &mut fan_controllers,
    );
    log::info!("T2 Fan Daemon is shutting down...");
    for mut fan in fan_controllers {
        fan.set_manual(false)?;
    }

    drop(lock);
    res
}
