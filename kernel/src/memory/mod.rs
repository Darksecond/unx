use bootinfo::boot_info::MemoryMap;

mod phys;

pub fn init(map: &MemoryMap) {
    phys::init(map);
}