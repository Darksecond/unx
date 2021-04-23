
cd bootloader-efi
cargo build
cd ..

cd kernel
cargo build
cd ..

"C:\Program Files\qemu\qemu-system-x86_64w.exe" -bios ovmf.fd -machine q35 -m 128M -drive format=raw,file=fat:rw:bootloader-efi/target/x86_64-unknown-uefi/debug