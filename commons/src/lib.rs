use std::env;
use std::fmt::Error;

pub mod packet;

pub const VERSION: &str = concat!("v", env!("VERGEN_SEMVER"), "-", env!("VERGEN_SHA_SHORT"));
pub const DEFAULT_PORT: u16 = 8765;
pub const DEFAULT_PORT_STR: &str = "8765";
pub const BUFFER_SIZE: usize = 2048;

pub fn get_path(num: usize) -> Result<String, Error> {
    if num > 9 {
        Err(Error::default())
    } else {
        Ok(format!(
            "{}discord-ipc-{}",
            if cfg!(windows) {
                "\\\\.\\pipe\\".to_string()
            } else {
                get_temp_path()? + "/"
            },
            num
        ))
    }
}

pub fn get_temp_path() -> Result<String, Error> {
    env::var("XDG_RUNTIME_DIR")
        .or_else(|_| env::var("TMPDIR"))
        .or_else(|_| env::var("TMP"))
        .or_else(|_| env::var("TEMP"))
        .or_else(|_| env::temp_dir().into_os_string().into_string())
        .or_else(|_| Ok("/tmp".to_string()))
}
