use std::process::Command;

use anyhow::Result;

#[cfg(target_os = "windows")]
const QEMU_PATH: &str = r"C:\Program Files\qemu\qemu-system-x86_64.exe";

#[cfg(not(target_os = "windows"))]
const QEMU_PATH: &str = "qemu-system-x86_64";

pub fn run() -> Result<()> {
    Command::new(QEMU_PATH)
        .arg("-nodefaults")
        .arg("-vga").arg("std")
        .arg("-serial").arg("stdio")
        .arg("-smp").arg("4")
        .arg("-monitor").arg("vc:1024x768")
        .arg("-bios").arg("ovmf.fd")
        .arg("-machine").arg("q35")
        .arg("-m").arg("256M")
        .arg("-drive").arg("format=raw,file=disk.img")
        .status()?;

    Ok(())
}
