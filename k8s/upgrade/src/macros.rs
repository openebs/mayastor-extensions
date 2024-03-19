/// Converts a list of expressions (each of whom implement ToString) to a Vec<String>.
#[macro_export]
macro_rules! vec_to_strings {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}
