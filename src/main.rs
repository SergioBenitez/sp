use std::process::ExitCode;
use std::ffi::{OsString, OsStr};
use std::cmp::max;

use os_str_bytes::RawOsStr;
use subprocess::{Exec, ExitStatus, PopenError};

const TEMPLATE_STR: &str = "{}";

fn templatize(template: &[OsString], args: &[OsString]) -> Vec<OsString> {
    let template = template.join(OsStr::new(" "));
    let template = template.to_str().expect("template must be UTF-8");
    args.iter()
        .map(|arg| arg.to_str().expect("template substitution must be UTF-8"))
        .map(|param| template.replace(TEMPLATE_STR, param))
        .map(|command| OsString::from(command))
        .collect::<Vec<_>>()
}

fn commands(args: &[OsString]) -> Vec<OsString> {
    fn is_template(args: &[OsString]) -> bool {
        args.iter().any(|a| RawOsStr::new(a).contains(TEMPLATE_STR))
    }

    let sections = args.split(|x| x == "::").collect::<Vec<_>>();
    match &sections[..] {
        &[tmpl, subs] if is_template(tmpl) => templatize(tmpl, subs),
        _ => sections.iter().map(|args| args.join(OsStr::new(" "))).collect(),
    }
}

fn main() -> ExitCode {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).map_or(false, |a| a == "-h" || a == "--help") {
        eprint!("{}", include_str!("../help.txt"));
        std::process::exit(0);
    }

    let mut threads = vec![];
    for command in commands(&args[1..]) {
        let cmd_str = command.clone();
        let thread = std::thread::spawn(|| Exec::shell(cmd_str).join());
        threads.push((command, thread));
    }

    let mut max_exit = 0;
    for (command, thread) in threads {
        let result = thread.join();
        match result {
            Err(e) => {
                eprintln!("internal error: thread panic: {e:?}");
                max_exit = max(max_exit, 127);
            }
            Ok(Ok(ExitStatus::Exited(code))) => {
                if code != 0 {
                    eprintln!("{command:?} exited with code {code:?}");
                }

                max_exit = max(max_exit, code);
            }
            Ok(Ok(ExitStatus::Signaled(i))) => {
                eprintln!("{command:?} interrupted: {i:?}");
            }
            Ok(Err(PopenError::IoError(e))) => {
                eprintln!("{command:?} I/O failed: {e}");
                max_exit = max(max_exit, 2);
            }
            Ok(Err(PopenError::LogicError(e))) => {
                eprintln!("{command:?} logic error: {e}");
                max_exit = max(max_exit, 3);
            },
            Ok(Ok(ExitStatus::Other(_))) | Ok(Ok(ExitStatus::Undetermined)) | Ok(Err(_)) => {
                eprintln!("{command:?} failed in an unexpected manner: {result:?}");
                max_exit = max(max_exit, 126);
            }
        }
    }

    ExitCode::from(max_exit as u8)
}
