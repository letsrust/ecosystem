use std::{fs::File, io::Read};

// match some_expression {
//     Ok(value) => value,
//     Err(err) => return Err(err),
// }
// <==>
// some_expression?

fn main() -> Result<(), std::io::Error> {
    let username = read_username("path/to/username.txt");
    println!("Username or error: {username:?}");

    let _username = read_username("path/to/username.txt")?;

    Ok(())
}

fn read_username(path: &str) -> Result<String, std::io::Error> {
    let username_file_result = File::open(path);
    let mut username_file: File = match username_file_result {
        Ok(file) => file,
        Err(err) => return Err(err),
    };

    let mut username = String::new();
    match username_file.read_to_string(&mut username) {
        Ok(_) => Ok(username),
        Err(err) => Err(err),
    }
}
