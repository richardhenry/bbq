mod cli;
mod config;
mod open;
mod theme;
mod tui;
mod update;

use std::io::{self, IsTerminal};

use clap::{CommandFactory, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse();

    if let Some(command) = cli.command {
        return cli::run_command(command);
    }

    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
    if is_tty {
        tui::run_tui()
    } else {
        cli::Cli::command().print_help()?;
        println!();
        Ok(())
    }
}
