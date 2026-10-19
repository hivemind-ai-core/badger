//! Shell completion scripts for `badger`.
//!
//! The scripts are written by hand (no completion generator, and no
//! dependency on one): they complete the names of the commands and the
//! main flags of each of them, so the user stops typing them.

/// A command of `badger`, with the flags it takes.
struct Cmd {
    /// The command's name, as it is typed.
    name: &'static str,
    /// What the command does, in one line.
    about: &'static str,
    /// The command's flags (the names in `FLAGS`).
    flags: &'static [&'static str],
}

/// A flag of a command: its name, what it does, and the name of the
/// value it takes (empty when it takes none).
struct Flag {
    name: &'static str,
    about: &'static str,
    value: &'static str,
}

/// Every flag the commands take.
const FLAGS: &[Flag] = &[
    Flag { name: "--json", about: "print the result as JSON", value: "" },
    Flag { name: "-o", about: "write the output to FILE", value: "FILE" },
    Flag { name: "--output", about: "write the output to FILE", value: "FILE" },
    Flag { name: "--profile", about: "merge the profile NAME into the configuration", value: "NAME" },
    Flag { name: "--workers", about: "run the tasks on N worker threads", value: "N" },
    Flag { name: "--opt", about: "MIR optimization level: release, aggressive, superopt", value: "LEVEL" },
    Flag { name: "--release", about: "optimize the generated code", value: "" },
    Flag { name: "--dump-mir", about: "print the MIR of the program", value: "" },
    Flag { name: "--watch", about: "recheck the sources when they change", value: "" },
    Flag { name: "--inspect", about: "serve inspection snapshots at ADDR", value: "ADDR" },
    Flag { name: "--reconstruct", about: "restart the program on an incompatible change", value: "" },
    Flag { name: "--bench", about: "report how long the program ran", value: "" },
    Flag { name: "--debug", about: "register debug information for a debugger", value: "" },
    Flag { name: "--record", about: "write a reproduction bundle to FILE", value: "FILE" },
    Flag { name: "--compare", about: "report the differences from the recording", value: "" },
    Flag { name: "--list", about: "list the selected tests instead of running them", value: "" },
    Flag { name: "--exact", about: "the filter names one test", value: "" },
    Flag { name: "--project", about: "the project directory", value: "DIR" },
    Flag { name: "--check", about: "report the differences instead of writing", value: "" },
    Flag { name: "--connect", about: "the address of a running program", value: "ADDR" },
    Flag { name: "--format", about: "the output format: dot, mermaid, json", value: "FORMAT" },
    Flag { name: "--std", about: "include the standard library", value: "" },
    Flag { name: "--line", about: "the line of the position (1-based)", value: "N" },
    Flag { name: "--col", about: "the column of the position (1-based)", value: "N" },
    Flag { name: "--source", about: "ask about this source text", value: "TEXT" },
    Flag { name: "--name", about: "the new name", value: "NAME" },
    Flag { name: "--overlays", about: "the proposed file contents", value: "FILE" },
    Flag { name: "--out", about: "write the pages into DIR", value: "DIR" },
    Flag { name: "--item", about: "the name of the generated record", value: "NAME" },
    Flag { name: "--dry-run", about: "print the plan and write nothing", value: "" },
    Flag { name: "--database", about: "the Postgres URL to take the types from", value: "URL" },
    Flag { name: "--schema", about: "the SQLite schema file", value: "FILE" },
    Flag { name: "--tests", about: "also run the tests", value: "" },
    Flag { name: "--lib", about: "make a library package", value: "" },
];

/// Every command, in the order `badger --help` lists them.
const COMMANDS: &[Cmd] = &[
    Cmd { name: "check", about: "parse and check a program", flags: &["--json"] },
    Cmd {
        name: "compile",
        about: "check a program and save it as binary MIR",
        flags: &["-o", "--output", "--profile", "--workers", "--opt"],
    },
    Cmd {
        name: "release",
        about: "write a self-contained release directory",
        flags: &["-o", "--output", "--profile", "--workers", "--opt"],
    },
    Cmd { name: "start", about: "start a release directory", flags: &["--workers"] },
    Cmd {
        name: "run",
        about: "compile a program with the JIT and run it",
        flags: &[
            "--release",
            "--dump-mir",
            "--opt",
            "--profile",
            "--watch",
            "--inspect",
            "--workers",
            "--reconstruct",
            "--bench",
            "--debug",
            "--record",
        ],
    },
    Cmd {
        name: "replay",
        about: "run a program again from a reproduction bundle",
        flags: &["--compare", "--release"],
    },
    Cmd {
        name: "test",
        about: "run the test functions of a program",
        flags: &["--json", "--list", "--exact"],
    },
    Cmd { name: "task", about: "run a user task of the project", flags: &["--project", "--profile"] },
    Cmd { name: "format", about: "rewrite the sources in the canonical layout", flags: &["--check"] },
    Cmd {
        name: "repl",
        about: "interactive session: definitions accumulate, expressions evaluate",
        flags: &["--connect"],
    },
    Cmd {
        name: "inspect",
        about: "inspect the program structure",
        flags: &["--json", "--profile", "--connect"],
    },
    Cmd { name: "update", about: "resolve the dependencies and rewrite badger.lock", flags: &[] },
    Cmd { name: "lsp", about: "language server over stdio", flags: &[] },
    Cmd { name: "mcp", about: "Model Context Protocol server over stdio", flags: &[] },
    Cmd { name: "graph", about: "the program structure as a graph", flags: &["--format", "--std"] },
    Cmd {
        name: "query",
        about: "answer one question about a program as JSON",
        flags: &["--line", "--col", "--source", "--name", "--overlays"],
    },
    Cmd { name: "doc", about: "reference documentation in Markdown", flags: &["--out", "--check"] },
    Cmd { name: "new", about: "create a project in a new directory", flags: &[] },
    Cmd {
        name: "generate",
        about: "generate typed code",
        flags: &["--item", "--dry-run", "--database", "--schema", "--check"],
    },
    Cmd {
        name: "contracts",
        about: "list the contract clauses and what was decided about them",
        flags: &["--json", "--tests"],
    },
    Cmd {
        name: "explain",
        about: "explain a diagnostic code, or the type of a definition",
        flags: &["--json"],
    },
    Cmd { name: "watch", about: "re-run a command when a source file changes", flags: &[] },
    Cmd { name: "init", about: "turn a directory into a Badger project", flags: &["--lib"] },
    Cmd { name: "doctor", about: "report the environment, for bug reports", flags: &["--json"] },
    Cmd { name: "completions", about: "print a shell completion script", flags: &[] },
    Cmd {
        name: "search",
        about: "find a definition in the standard library and the project",
        flags: &["--json"],
    },
];

/// The flag `name`, with what it does.
fn flag(name: &str) -> Option<&'static Flag> {
    FLAGS.iter().find(|f| f.name == name)
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
fn command_names() -> String {
    COMMANDS.iter().map(|c| c.name).collect::<Vec<_>>().join(" ")
}

/// The script for bash.
fn bash() -> String {
    let mut s = String::new();
    s.push_str(
        "# badger completion for bash.\n\
         #\n\
         # Install:  source <(badger completions bash)\n\
         # Put that line in ~/.bashrc to load it in every new shell.\n\
         \n\
         _badger_complete() {\n\
         \x20   local cur prev cmd flags i\n\
         \x20   COMPREPLY=()\n\
         \x20   cur=\"${COMP_WORDS[COMP_CWORD]}\"\n\
         \x20   prev=\"${COMP_WORDS[COMP_CWORD-1]}\"\n\
         \n\
         \x20   # the command: the first word that is not a flag\n\
         \x20   cmd=\"\"\n\
         \x20   for (( i=1; i < COMP_CWORD; i++ )); do\n\
         \x20       case \"${COMP_WORDS[i]}\" in\n\
         \x20           -*) ;;\n\
         \x20           *) cmd=\"${COMP_WORDS[i]}\"; break ;;\n\
         \x20       esac\n\
         \x20   done\n\
         \n\
         \x20   flags=\"--help --version\"\n\
         \x20   case \"$cmd\" in\n",
    );
    for c in COMMANDS {
        s.push_str(&format!("        {}) flags=\"$flags", c.name));
        for f in c.flags {
            s.push_str(&format!(" {f}"));
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
    s.push_str(&command_names());
    s.push_str(
        "\" -- \"$cur\") )\n\
         \x20       return 0\n\
         \x20   fi\n\
         \n\
         \x20   # the values of the flags that have a fixed set\n\
         \x20   case \"$prev\" in\n\
         \x20       --opt) COMPREPLY=( $(compgen -W \"release aggressive superopt\" -- \"$cur\") ); return 0 ;;\n\
         \x20       --format) COMPREPLY=( $(compgen -W \"dot mermaid json\" -- \"$cur\") ); return 0 ;;\n\
         \x20       --shell) COMPREPLY=( $(compgen -W \"bash zsh fish powershell\" -- \"$cur\") ); return 0 ;;\n\
         \x20   esac\n\
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
    for c in COMMANDS {
        s.push_str(&format!("        '{}:{}'\n", c.name, c.about));
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
    for c in COMMANDS {
        s.push_str(&format!("                {}) ", c.name));
        if c.flags.is_empty() {
            s.push_str("_files ;;\n");
            continue;
        }
        s.push_str("_arguments");
        for f in c.flags {
            let Some(flag) = flag(f) else { continue };
            if flag.value.is_empty() {
                s.push_str(&format!(" '{}[{}]'", flag.name, flag.about));
            } else {
                s.push_str(&format!(" '{}[{}]:{}:'", flag.name, flag.about, flag.value));
            }
        }
        s.push_str(" '*:file:_files' ;;\n");
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
    let mut s = String::new();
    s.push_str(
        "# badger completion for fish.\n\
         #\n\
         # Install:  badger completions fish > ~/.config/fish/completions/badger.fish\n\
         \n\
         complete -c badger -f\n",
    );
    for c in COMMANDS {
        s.push_str(&format!(
            "complete -c badger -n '__fish_use_subcommand' -a '{}' -d '{}'\n",
            c.name, c.about
        ));
        for f in c.flags {
            let Some(flag) = flag(f) else { continue };
            let long = flag.name.strip_prefix("--").unwrap_or("");
            let short = if long.is_empty() { flag.name.strip_prefix('-').unwrap_or("") } else { "" };
            let mut line = format!("complete -c badger -n '__fish_seen_subcommand_from {}' ", c.name);
            if !long.is_empty() {
                line.push_str(&format!("-l {long} "));
            }
            if !short.is_empty() {
                line.push_str(&format!("-s {short} "));
            }
            if !flag.value.is_empty() {
                line.push_str("-r ");
            }
            line.push_str(&format!("-d '{}'\n", flag.about));
            s.push_str(&line);
        }
    }
    s
}

/// The script for PowerShell.
fn powershell() -> String {
    let mut s = String::new();
    s.push_str(
        "# badger completion for PowerShell.\n\
         #\n\
         # Install:  badger completions powershell | Out-String | Invoke-Expression\n\
         # (or put that line in your PowerShell profile).\n\
         \n\
         Register-ArgumentCompleter -Native -CommandName 'badger' -ScriptBlock {\n\
         \x20   param($wordToComplete, $commandAst, $cursorPosition)\n\
         \n\
         \x20   $commands = @(\n",
    );
    for c in COMMANDS {
        s.push_str(&format!("        '{}',\n", c.name));
    }
    s.push_str("    )\n");
    s.push_str("    $flags = @{\n");
    for c in COMMANDS {
        s.push_str(&format!("        '{}' = @(", c.name));
        for (i, f) in c.flags.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("'{f}'"));
        }
        s.push_str(")\n");
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
