use clap::CommandFactory;
use clap_complete::{
    generate,
    shells::{Bash, Elvish, Fish, PowerShell, Zsh},
};

use crate::cli::args::{Cli, CompletionArgs, CompletionShell, OutputMode};

pub(crate) fn run(args: CompletionArgs, output: OutputMode) -> anyhow::Result<()> {
    if output != OutputMode::Human {
        return Err(crate::cli::output::usage_error(
            "shell completion source is available only with human output",
        ));
    }

    let mut command = Cli::command();
    let binary_name = command.get_name().to_string();
    let mut stdout = std::io::stdout();
    match args.shell {
        CompletionShell::Bash => generate(Bash, &mut command, binary_name, &mut stdout),
        CompletionShell::Zsh => generate(Zsh, &mut command, binary_name, &mut stdout),
        CompletionShell::Fish => generate(Fish, &mut command, binary_name, &mut stdout),
        CompletionShell::Powershell => generate(PowerShell, &mut command, binary_name, &mut stdout),
        CompletionShell::Elvish => generate(Elvish, &mut command, binary_name, &mut stdout),
    }
    Ok(())
}
