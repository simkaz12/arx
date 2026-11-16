mod driver;
mod editor;
mod format;
mod piped;
mod scenario;
mod snapshot;
mod state;
mod tty;

use std::io::IsTerminal;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let auto = args.iter().any(|a| a == "--auto");
    let fast = args.iter().any(|a| a == "--fast");
    let snapshot = args.iter().any(|a| a == "--snapshot");

    if snapshot {
        snapshot::run(fast);
        return;
    }

    if std::io::stdout().is_terminal() {
        tty::run(auto, fast);
    } else {
        piped::run(auto, fast);
    }
}
