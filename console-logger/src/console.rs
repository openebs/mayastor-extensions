use nu_ansi_term::Color::{Cyan, LightBlue, Red};

/// Print info on console.
pub fn info(message: &str) {
    println!("{}", Cyan.bold().italic().paint(message));
}

/// Print warning on console.
pub fn warn(message: &str, data: &str) {
    println!(
        "{} {}",
        Red.bold().italic().paint(message),
        LightBlue.bold().italic().paint(data)
    );
}
