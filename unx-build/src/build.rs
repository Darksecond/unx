use anyhow::Result;
use std::process::Command;

pub fn build() -> Result<()> {
    println!("Building...");

    cargo("kernel")?;
    cargo("bootloader")?;

    FatBuilder::new("disk.fat")
        .file("kernel/target/x86_64-unx/release/kernel", "kernel.elf")
        .file(
            "bootloader/target/x86_64-unknown-uefi/release/bootloader.efi",
            "efi/boot/bootx64.efi",
        )
        .build()?;

    build_gpt("disk.fat", "disk.img")?;

    Ok(())
}

pub fn cargo(cwd: &str) -> Result<()> {
    println!("Building {}...", cwd);
    println!("Running `cargo build --release`");

    Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(cwd)
        .status()?;

    Ok(())
}

struct FatBuilder {
    disk_path: String,
    files: Vec<(String, String)>,
}

impl FatBuilder {
    pub fn new(disk_path: &str) -> Self {
        Self {
            disk_path: disk_path.to_owned(),
            files: vec![],
        }
    }

    pub fn file(&mut self, source: &str, destination: &str) -> &mut Self {
        self.files.push((source.to_owned(), destination.to_owned()));
        self
    }

    pub fn build(&mut self) -> Result<()> {
        use anyhow::Context;
        use std::fs;
        use std::io;

        const MB: u64 = 1024 * 1024;

        let mut total_size = 0;
        for file in &self.files {
            let metadata = fs::metadata(&file.0)?;
            total_size += metadata.len();
        }

        let total_size = ((total_size - 1) / MB + 1) * MB;
        println!("Disk size: {}MB", total_size / MB);

        let fat_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.disk_path)?;

        fat_file.set_len(total_size)?;

        let options = fatfs::FormatVolumeOptions::new().volume_label(*b"FOOO       ");
        fatfs::format_volume(&fat_file, options)?;

        let partition = fatfs::FileSystem::new(&fat_file, fatfs::FsOptions::new())?;

        let root = partition.root_dir();

        for file in &self.files {
            let mut dir = root.clone();
            let path = std::path::Path::new(&file.1);
            for component in path.parent().context("path incorrect")?.components() {
                dir = dir.create_dir(
                    component
                        .as_os_str()
                        .to_str()
                        .context("cannot convert to string")?,
                )?;
            }

            let mut fat_file = root.create_file(&file.1)?;
            fat_file.truncate()?;
            io::copy(&mut fs::File::open(&file.0)?, &mut fat_file)?;
        }

        Ok(())
    }
}

pub fn build_gpt(fat_path: &str, gpt_path: &str) -> Result<()> {
    use anyhow::Context;
    use std::convert::TryFrom;
    use std::fs;
    use std::fs::File;
    use std::io;
    use std::io::Seek;

    let mut image = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(gpt_path)?;

    let partition_size = fs::metadata(fat_path)?.len();
    let image_size = partition_size + 1024 * 64;
    image.set_len(image_size)?;

    let mbr = gpt::mbr::ProtectiveMBR::with_lb_size(
        u32::try_from((image_size / 512) - 1).unwrap_or(0xFF_FF_FF_FF),
    );
    mbr.overwrite_lba0(&mut image)?;

    let block_size = gpt::disk::LogicalBlockSize::Lb512;

    let mut disk = gpt::GptConfig::new()
        .writable(true)
        .initialized(false)
        .logical_block_size(block_size)
        .create_from_device(Box::new(&mut image), None)?;

    disk.update_partitions(Default::default())?;

    let partition_id = disk.add_partition("boot", partition_size, gpt::partition_types::EFI, 0)?;

    let partition = disk
        .partitions()
        .get(&partition_id)
        .context("Cannot find partition")?;

    let start_offset = partition.bytes_start(block_size)?;
    disk.write()?;

    image.seek(io::SeekFrom::Start(start_offset))?;

    io::copy(&mut File::open(&fat_path)?, &mut image)?;

    Ok(())
}
