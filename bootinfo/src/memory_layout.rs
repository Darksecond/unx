
pub const KERNEL_SPACE_BASE: u64 = 0xFFFF_FF80_0000_0000; //-512GiB
pub const KERNEL_BASE: u64       = 0xFFFF_FFFF_8000_0000; //-2GiB

pub const PHYSMAP_BASE: u64 = KERNEL_SPACE_BASE;
pub const PHYSMAP_SIZE: u64 = gibibyte(256);
pub const PHYSMAP_TOP: u64 = PHYSMAP_BASE + PHYSMAP_SIZE;

pub const STACK_GUARD: u64 = 0xFFFF_FFC0_0000_0000; //right after physmap
pub const STACK_BASE: u64 = STACK_GUARD + page(1);
pub const STACK_FRAMES: u64 = 20;
pub const STACK_SIZE: u64 = page(STACK_FRAMES); //80KiB
pub const STACK_TOP: u64 = STACK_BASE + STACK_SIZE;

//bootinfo area also contains framebuffer, console font, etc
pub const BOOTINFO_BASE: u64 = 0xFFFF_FFC0_8000_0000; //2 gigs after STACK_GUARD
pub const BOOTINFO_SIZE: u64 = gibibyte(1);
pub const BOOTINFO_TOP: u64 = BOOTINFO_BASE + BOOTINFO_SIZE;

const fn page(num: u64) -> u64 {
    num * 0x1000
}

#[allow(dead_code)]
const fn kibobyte(num: u64) -> u64 {
    num * 1024
}

#[allow(dead_code)]
const fn mibibyte(num: u64) -> u64 {
    num * 1024 * 1024
}

const fn gibibyte(num: u64) -> u64 {
    num * 1024 * 1024 * 1024
}