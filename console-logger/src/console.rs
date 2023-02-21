use nu_ansi_term::Color::{Cyan, Red};

/// Print info on console.
pub fn info(message: &str) {
    println!("{}", Cyan.bold().italic().paint(message));
}

/// Print warning on console.
pub fn warn(data: &str, message: &str) {
    println!(
        "{} {} ",
        Red.bold().italic().paint(message),
        Cyan.bold().italic().paint(data)
    );
}
