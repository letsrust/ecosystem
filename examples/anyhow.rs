use anyhow::{Ok, Result};

fn main() -> Result<()> {
    let config = std::fs::read_to_string("Cargo.toml")?;
    println!("{}", config);
    // let map: ClusterMap = serde_json::from_str(&config)?;
    Ok(())
}
