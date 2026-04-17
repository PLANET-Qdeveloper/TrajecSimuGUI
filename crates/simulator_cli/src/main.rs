use clap::Parser;
use simulator_core::{CustomSimulator, Simulator, parameters::SimulationParams};
use std::fs;

#[derive(Parser, Debug)]
#[command(name = "Rocket Simulator CLI")]
#[command(about = "Command line interface for rocket trajectory simulation", long_about = None)]
struct Args {
    /// Input parameters JSON file
    #[arg(short, long)]
    input: Option<String>,

    /// Output results CSV file
    #[arg(short, long)]
    output: Option<String>,

    /// Initial altitude (m)
    #[arg(long, default_value = "0")]
    altitude: f64,

    /// Initial velocity (m/s)
    #[arg(long, default_value = "100")]
    velocity: f64,

    /// Launch angle (degrees)
    #[arg(long, default_value = "45")]
    angle: f64,

    /// Simulation time (s)
    #[arg(long, default_value = "100")]
    time: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();

    // Load or create simulation parameters
    let mut params = if let Some(input_file) = args.input {
        let content = fs::read_to_string(input_file)?;
        serde_json::from_str(&content)?
    } else {
        SimulationParams {
            initial_altitude: args.altitude,
            initial_velocity: args.velocity,
            initial_pitch: args.angle,
            max_time: args.time,
            ..Default::default()
        }
    };

    // Create simulator
    let mut simulator = CustomSimulator::new();
    simulator.initialize(&params)?;

    // Run simulation
    let mut results = Vec::new();

    while !simulator.is_complete() {
        simulator.step(params.time_step)?;
        let output = simulator.get_output()?;
        results.push(output);
    }

    // Output results
    if let Some(output_file) = args.output {
        let json = serde_json::to_string_pretty(&results)?;
        fs::write(output_file, json)?;
    } else {
        println!("Simulation completed. Total steps: {}", results.len());
        if let Some(last) = results.last() {
            println!("Final altitude: {:.2} m", last.state.position.z);
            println!("Max altitude: {:.2} m", last.max_altitude);
            println!("Downrange: {:.2} m", last.downrange_distance);
        }
    }

    Ok(())
}
