# UNX Operating System Project

This project aims to build a simple kernel in rust for research purposes.
It supports UEFI boot on x86_64.

## Building & Running

A build tool is included, aliased as `cargo unx`. 
You can use `cargo unx build` in the top-level directory to build the project.
A `disk.img` file will result, containing a bootloader, kernel all ready for operation.

`cargo unx run` is provided to build, then run the OS using QEMU.

# Required Dependencies

You need the following dependencies installed:

- A recent rust nightly
- `rust-src` & `llvm-tools-preview` components
- `qemu-system-x86_64` installed