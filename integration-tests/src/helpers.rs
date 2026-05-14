macro_rules! step {
    ($tag:expr, $($arg:tt)*) => {
        println!("• [{}] {}", $tag, format!($($arg)*));
    };
}

pub(crate) use step;
