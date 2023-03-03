use nu_ansi_term::Color::{Cyan, Red};

/// Print info on console.
pub fn info(message: &str, data: &str) {
    println!(
        "{} \n {} ",
        Cyan.bold().italic().paint(message),
        Cyan.bold().italic().paint(data)
    );
}

/// Print warning on console.
pub fn warn(message: &str, data: &str) {
    println!(
        "{} \n {} ",
        Cyan.bold().italic().paint(message),
        Red.bold().italic().paint(data)
    );
}
