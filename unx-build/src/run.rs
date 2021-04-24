use std::process::Command;

use anyhow::Result;

pub fn run() -> Result<()> {

  Command::new(r"C:\Program Files\qemu\qemu-system-x86_64w.exe")
  .arg("-nodefaults")
  .arg("-vga").arg("std")
  // .arg("-serial").arg("stdio")
  // .arg("-smp").arg("4")
  .arg("-monitor").arg("vc:1024x768")
  .arg("-bios").arg("ovmf.fd")
  .arg("-machine").arg("q35")
  .arg("-m").arg("256M")
  .arg("-drive").arg("format=raw,file=disk.img")
  .status()?;

  Ok(())
}