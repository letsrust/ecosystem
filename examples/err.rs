use std::{fs::File, io::Read, mem::size_of};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),

    #[error("Serialize json error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

#[allow(unused)]
pub struct BigError {
    a: String,
    b: Vec<String>,
    c: [u8; 32],
    d: u64,
}

fn main() -> Result<(), anyhow::Error> {
    println!("Hello, world!");

    println!("size of anyhow::Error is {}", size_of::<anyhow::Error>());
    println!("size of std::io::Error is {}", size_of::<std::io::Error>());
    println!(
        "size of std::num::ParseIntError is {}",
        size_of::<std::num::ParseIntError>()
    );
    println!(
        "size of serde_json::Error is {}",
        size_of::<serde_json::Error>()
    );
    println!("size of String is {}", size_of::<String>());
    println!("size of MyError is {}", size_of::<MyError>());

    result_demo();

    fail_with_error()?;

    Ok(())
}

fn fail_with_error() -> Result<(), MyError> {
    Err(MyError::Custom("This is a custom error".to_string()))
}

fn result_demo() {
    let file: Result<File, std::io::Error> = File::open("Cargo.toml");
    match file {
        Ok(mut file) => {
            let mut contents = String::new();
            if let Ok(bytes) = file.read_to_string(&mut contents) {
                println!("Read {} bytes", bytes);
            } else {
                println!("Failed to read the file");
            }
        }
        Err(err) => {
            println!("Open the file: {err}");
        }
    }
}
