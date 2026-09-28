//! Classify a live text snapshot supplied by the host on standard input.

use std::io::{self, Read};

use agentsense::{parse_agent_label, DetectionInput, Detector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let label = std::env::args()
        .nth(1)
        .ok_or("usage: read_screen <agent> < screen.txt")?;
    let agent = parse_agent_label(&label).ok_or("unrecognized agent label")?;
    let mut screen = String::new();
    io::stdin().read_to_string(&mut screen)?;
    let explanation = Detector::bundled().explain(Some(agent), DetectionInput::screen(&screen));
    println!("{}", serde_json::to_string_pretty(&explanation)?);
    Ok(())
}
