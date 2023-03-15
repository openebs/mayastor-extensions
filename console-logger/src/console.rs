use nu_ansi_term::Color::{Cyan, Red};

/// Print info on console.
pub fn info(message: &str) {
    println!("{}", Cyan.bold().italic().paint(message));
}

/// Print warning on console.
pub fn warn(message: &str, data: &str) {
    println!(
        "{} \n {} ",
        Cyan.bold().italic().paint(message),
        Red.bold().italic().paint(data)
    );
}
