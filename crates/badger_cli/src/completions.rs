//! Shell completion scripts for `badger`.
//!
//! The scripts are written by hand (no completion generator, and no
//! dependency on one), but what they complete is read from the argument
//! parser: the commands, their flags, and the values those flags and
//! arguments take. So they cannot drift from `badger --help`.

use clap::{CommandFactory, builder::Arg};

use crate::Cli;

/// A command of `badger`, with what it completes.
struct Cmd {
    /// The command's name, as it is typed.
    name: String,
    /// What the command does, in one line.
    about: String,
    /// The command's flags.
    flags: Vec<Flag>,
    /// The values the command's first argument takes, when it takes only
    /// a fixed set.
    values: Vec<String>,
}

/// A flag of a command.
struct Flag {
    /// Its names: the long one, and the short one when it has one.
    names: Vec<String>,
    /// What it does, in one line.
    about: String,
    /// The name of the value it takes (empty when it takes none).
    value: String,
    /// The values it takes, when it takes only a fixed set.
    values: Vec<String>,
}

/// The parser, with what it adds itself (`build` adds the `help`
/// command, the `--help` and `--version` flags).
fn parser() -> clap::Command {
    let mut cli = Cli::command();
    cli.build();
    cli
}

/// The commands of `badger`, as the parser holds them.
fn commands() -> Vec<Cmd> {
    parser()
        .get_subcommands()
        .map(|c| Cmd {
            name: c.get_name().to_string(),
            about: c.get_about().map(|a| a.to_string()).unwrap_or_default(),
            flags: c.get_arguments().filter(|a| !a.is_positional() && !a.is_hide_set()).map(flag).collect(),
            values: c.get_arguments().find(|a| a.is_positional()).map(values).unwrap_or_default(),
        })
        .collect()
}

/// The global flags of `badger` itself.
fn global_flags() -> Vec<Flag> {
    parser().get_arguments().filter(|a| !a.is_positional() && !a.is_hide_set()).map(flag).collect()
}

/// A flag, as the scripts complete it.
fn flag(a: &Arg) -> Flag {
    let mut names = vec![];
    if let Some(long) = a.get_long() {
        names.push(format!("--{long}"));
    }
    if let Some(short) = a.get_short() {
        names.push(format!("-{short}"));
    }
    // a flag that takes no value (a switch) has no value name
    let takes = a.get_action().takes_values();
    Flag {
        names,
        about: a.get_help().map(|h| h.to_string()).unwrap_or_default(),
        value: if takes {
            a.get_value_names().and_then(|v| v.first()).map(|v| v.to_string()).unwrap_or_default()
        } else {
            String::new()
        },
        values: values(a),
    }
}

/// The values an argument takes, when it takes only a fixed set.
fn values(a: &Arg) -> Vec<String> {
    a.get_possible_values().iter().filter(|v| !v.is_hide_set()).map(|v| v.get_name().to_string()).collect()
}

/// The text as a shell word in single quotes.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The text as a fish description (single quotes, escaped with a
/// backslash).
fn fish_quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"\'"))
}

/// The text as a zsh `_arguments` description: the brackets around it
/// belong to the syntax, so they are escaped.
fn zsh_description(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]")
}

/// The completion script for `shell` (`None` for an unknown shell).
pub fn script(shell: &str) -> Option<String> {
    match shell {
        "bash" => Some(bash()),
        "zsh" => Some(zsh()),
        "fish" => Some(fish()),
        "powershell" => Some(powershell()),
        _ => None,
    }
}

/// The names of the commands, separated by spaces.
fn command_names(cmds: &[Cmd]) -> String {
    cmds.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(" ")
}

/// The flags that take a fixed set of values, in the order they first
/// appear (the same flag in two commands is listed once).
fn flag_values(cmds: &[Cmd]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = vec![];
    for c in cmds {
        for f in &c.flags {
            if f.values.is_empty() {
                continue;
            }
            let name = f.names.first().cloned().unwrap_or_default();
            if !out.iter().any(|(n, _)| n == &name) {
                out.push((name, f.values.clone()));
            }
        }
    }
    out
}

/// The script for bash.
fn bash() -> String {
    let cmds = commands();
    let mut s = String::new();
    s.push_str(
        "# badger completion for bash.\n\
         #\n\
         # Install:  source <(badger completions bash)\n\
         # Put that line in ~/.bashrc to load it in every new shell.\n\
         \n\
         _badger_complete() {\n\
         \x20   local cur prev cmd cmd_i flags i n\n\
         \x20   COMPREPLY=()\n\
         \x20   cur=\"${COMP_WORDS[COMP_CWORD]}\"\n\
         \x20   prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n\
         \x20   \n\
         \x20   # the command: the first word that is not a flag\n\
         \x20   cmd=\"\"\n\
         \x20   cmd_i=0\n\
         \x20   for (( i=1; i < COMP_CWORD; i++ )); do\n\
         \x20       case \"${COMP_WORDS[i]}\" in\n\
         \x20           -*) ;;\n\
         \x20           *) cmd=\"${COMP_WORDS[i]}\"; cmd_i=$i; break ;;\n\
         \x20       esac\n\
         \x20   done\n\
         \x20   \n\
         \x20   # the arguments already given after the command\n\
         \x20   n=0\n\
         \x20   for (( i=cmd_i+1; i < COMP_CWORD; i++ )); do\n\
         \x20       case \"${COMP_WORDS[i]}\" in\n\
         \x20           -*) ;;\n\
         \x20           *) n=$((n+1)) ;;\n\
         \x20       esac\n\
         \x20   done\n\
         \x20   \n\
         \x20   flags=\"",
    );
    s.push_str(&global_flags().iter().flat_map(|f| f.names.clone()).collect::<Vec<_>>().join(" "));
    s.push_str(
        "\"\n\
         \x20   case \"$cmd\" in\n",
    );
    for c in &cmds {
        s.push_str(&format!("        {}) flags=\"$flags", c.name));
        for f in &c.flags {
            for n in &f.names {
                s.push_str(&format!(" {n}"));
            }
        }
        s.push_str("\" ;;\n");
    }
    s.push_str(
        "    esac\n\
         \n\
         \x20   if [[ \"$cur\" == -* ]]; then\n\
         \x20       COMPREPLY=( $(compgen -W \"$flags\" -- \"$cur\") )\n\
         \x20       return 0\n\
         \x20   fi\n\
         \n\
         \x20   if [[ -z \"$cmd\" ]]; then\n\
         \x20       COMPREPLY=( $(compgen -W \"",
    );
    s.push_str(&command_names(&cmds));
    s.push_str(
        "\" -- \"$cur\") )\n\
         \x20       return 0\n\
         \x20   fi\n\
         \n\
         \x20   # the values of a flag that has a fixed set\n\
         \x20   case \"$prev\" in\n",
    );
    for (name, values) in flag_values(&cmds) {
        s.push_str(&format!(
            "        {name}) COMPREPLY=( $(compgen -W \"{}\" -- \"$cur\") ); return 0 ;;\n",
            values.join(" ")
        ));
    }
    s.push_str(
        "    esac\n\
         \n\
         \x20   # the values of a first argument that has a fixed set\n\
         \x20   if [[ $n -eq 0 ]]; then\n\
         \x20       case \"$cmd\" in\n",
    );
    for c in &cmds {
        if c.values.is_empty() {
            continue;
        }
        s.push_str(&format!(
            "        {}) COMPREPLY=( $(compgen -W \"{}\" -- \"$cur\") ); return 0 ;;\n",
            c.name,
            c.values.join(" ")
        ));
    }
    s.push_str(
        "        esac\n\
         \x20   fi\n\
         \n\
         \x20   COMPREPLY=( $(compgen -f -- \"$cur\") )\n\
         }\n\
         \n\
         complete -F _badger_complete badger\n",
    );
    s
}

/// The script for zsh.
fn zsh() -> String {
    let cmds = commands();
    let mut s = String::new();
    s.push_str(
        "# badger completion for zsh.\n\
         #\n\
         # Install:  badger completions zsh > \"${fpath[1]}/_badger\"\n\
         # Then start a new shell (or run `compinit`).\n\
         \n\
         _badger() {\n\
         \x20   local -a commands\n\
         \x20   commands=(\n",
    );
    for c in &cmds {
        s.push_str(&format!("        {}:{}\n", quoted(&c.name), quoted(&c.about)));
    }
    s.push_str(
        "    )\n\
         \n\
         \x20   _arguments -C '1: :->command' '*:: :->args' || return 1\n\
         \x20   case \"$state\" in\n\
         \x20       command)\n\
         \x20           _describe 'command' commands\n\
         \x20           ;;\n\
         \x20       args)\n\
         \x20           case \"${words[1]}\" in\n",
    );
    for c in &cmds {
        s.push_str(&format!("                {}) ", c.name));
        if c.flags.is_empty() && c.values.is_empty() {
            s.push_str("_files ;;\n");
            continue;
        }
        s.push_str("_arguments");
        for f in &c.flags {
            for n in &f.names {
                if f.value.is_empty() {
                    s.push_str(&format!(" '{}[{}]'", n, zsh_description(&f.about)));
                } else {
                    s.push_str(&format!(" '{}[{}]:{}:'", n, zsh_description(&f.about), f.value));
                }
            }
        }
        if c.values.is_empty() {
            s.push_str(" '*:file:_files'");
        } else {
            s.push_str(&format!(" '1:({})'", c.values.join(" ")));
            s.push_str(" '*:file:_files'");
        }
        s.push_str(" ;;\n");
    }
    s.push_str(
        "                *) _files ;;\n\
         \x20           esac\n\
         \x20           ;;\n\
         \x20   esac\n\
         }\n\
         \n\
         compdef _badger badger\n",
    );
    s
}

/// The script for fish.
fn fish() -> String {
    let cmds = commands();
    let mut s = String::new();
    s.push_str(
        "# badger completion for fish.\n\
         #\n\
         # Install:  badger completions fish > ~/.config/fish/completions/badger.fish\n\
         \n\
         complete -c badger -f\n",
    );
    for c in &cmds {
        s.push_str(&format!(
            "complete -c badger -n '__fish_use_subcommand' -a {} -d {}\n",
            fish_quoted(&c.name),
            fish_quoted(&c.about)
        ));
        for f in &c.flags {
            let long = f.names.iter().find(|n| n.starts_with("--"));
            let short = f.names.iter().find(|n| n.starts_with('-') && !n.starts_with("--"));
            let mut line = format!("complete -c badger -n '__fish_seen_subcommand_from {}' ", c.name);
            if let Some(long) = long {
                line.push_str(&format!("-l {} ", long.trim_start_matches("--")));
            }
            if let Some(short) = short {
                line.push_str(&format!("-s {} ", short.trim_start_matches('-')));
            }
            if !f.value.is_empty() {
                line.push_str("-r ");
            }
            line.push_str(&format!("-d {}\n", fish_quoted(&f.about)));
            s.push_str(&line);
        }
        if !c.values.is_empty() {
            for v in &c.values {
                s.push_str(&format!(
                    "complete -c badger -n '__fish_seen_subcommand_from {}' -a {}\n",
                    c.name,
                    fish_quoted(v)
                ));
            }
        }
    }
    s
}

/// The script for PowerShell.
fn powershell() -> String {
    let cmds = commands();
    let mut s = String::new();
    s.push_str(
        "# badger completion for PowerShell.\n\
         #\n\
         # Install:  badger completions powershell | Out-String | Invoke-Expression\n\
         # (or put that line in your PowerShell profile).\n\
         \n\
         Register-ArgumentCompleter -Native -CommandName 'badger' -ScriptBlock {\n\
         \x20   param($wordToComplete, $commandAst, $cursorPosition)\n\
         \x20   \n\
         \x20   $commands = @(\n",
    );
    for c in &cmds {
        s.push_str(&format!("        '{}',\n", c.name));
    }
    s.push_str("    )\n");
    s.push_str("    $flags = @{\n");
    for c in &cmds {
        s.push_str(&format!("        '{}' = @(", c.name));
        for (i, f) in c.flags.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("'{}'", f.names.join("', '")));
        }
        s.push_str(")\n");
    }
    s.push_str("    }\n");
    s.push_str("    $values = @{\n");
    for (name, values) in flag_values(&cmds) {
        s.push_str(&format!("        '{}' = @('{}')\n", name, values.join("', '")));
    }
    s.push_str(
        "    }\n\
         \n\
         \x20   $command = $null\n\
         \x20   foreach ($element in @($commandAst.CommandElements | Select-Object -Skip 1)) {\n\
         \x20       if (\"$element\" -notlike '-*') { $command = \"$element\"; break }\n\
         \x20   }\n\
         \n\
         \x20   $candidates = @()\n\
         \x20   if (-not $command) {\n\
         \x20       $candidates = $commands\n\
         \x20   } elseif ($values.ContainsKey($wordToComplete)) {\n\
         \x20       $candidates = $values[$wordToComplete]\n\
         \x20   } elseif ($flags.ContainsKey($command)) {\n\
         \x20       $candidates = $flags[$command]\n\
         \x20   }\n\
         \n\
         \x20   $candidates | Where-Object { $_ -like \"$wordToComplete*\" } | ForEach-Object {\n\
         \x20       [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterName', $_)\n\
         \x20   }\n\
         }\n",
    );
    s
}
