//! Inspect one local original map member; output is for local diagnostics only.
use pcx_map::Map;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("map path required")?;
    let member = args.next().ok_or("member path required")?;
    let data = std::fs::read(&path)?;
    let map = Map::parse(&data)?;
    let bytes = map.extract(&member)?;
    for byte in bytes {
        print!("{byte:02x}");
    }
    println!();
    Ok(())
}
