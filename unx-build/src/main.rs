use anyhow::Result;
use build::build;
use clap::{App, AppSettings, SubCommand};
use run::run;

mod build;
mod run;

//TODO clean subcommand

fn main() -> Result<()> {
    let matches = App::new("unx-build")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("build").about("Builds the entire os and generates a disk image"),
        )
        .subcommand(
            SubCommand::with_name("run").about("Builds then runs the disk image in qemu")
        )
        .get_matches();

    if let Some(_matches) = matches.subcommand_matches("build") {
        build()?;
    } else if let Some(_matches) = matches.subcommand_matches("run") {
        build()?;
        println!("Running...");
        run()?;
    }

    Ok(())
}
