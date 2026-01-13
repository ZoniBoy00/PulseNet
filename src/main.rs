use anyhow::Result;
use chrono::Local;
use clap::Parser;
use colored::*;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Parser, Debug)]
#[command(author, version, about = "⚡ PulseNet - Ultra-fast IP Discovery Tool")]
struct Args {
    /// Total number of IPs to scan
    #[arg(short, long, default_value_t = 1000)]
    count: u32,

    /// Timeout for each connection in milliseconds
    #[arg(short, long, default_value_t = 800)]
    timeout: u64,

    /// Number of concurrent workers (Higher = faster, but can saturate connection)
    #[arg(short = 'w', long, default_value_t = 64)]
    workers: usize,

    /// Ports to check (comma separated)
    #[arg(short, long, default_value = "80,443,22,8080")]
    ports: String,

    /// Output file to save working IPs
    #[arg(short, long, default_value = "pulse_results.log")]
    output: String,

    /// Skip checking and just generate random IPs
    #[arg(short, long)]
    generate_only: bool,
}

struct Scanner {
    ports: Vec<u16>,
    timeout_ms: u64,
}

impl Scanner {
    fn new(ports_str: &str, timeout_ms: u64) -> Self {
        let ports = ports_str
            .split(',')
            .filter_map(|s| s.trim().parse::<u16>().ok())
            .collect();
        Self { ports, timeout_ms }
    }

    fn generate_random_ip() -> Ipv4Addr {
        let mut rng = rand::thread_rng();
        Ipv4Addr::new(
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
        )
    }

    async fn check_ip_details(&self, ip: Ipv4Addr) -> Option<u16> {
        let mut tasks = Vec::new();
        for &port in &self.ports {
            let addr = SocketAddr::new(ip.into(), port);
            let timeout_ms = self.timeout_ms;
            tasks.push(Box::pin(async move {
                let connect_result = timeout(Duration::from_millis(timeout_ms), TcpStream::connect(addr)).await;
                if let Ok(Ok(_)) = connect_result {
                    Some(port)
                } else {
                    None
                }
            }));
        }

        let mut tasks = tasks;
        while !tasks.is_empty() {
            let (result, _index, remaining) = futures::future::select_all(tasks).await;
            if result.is_some() {
                return result;
            }
            tasks = remaining;
        }
        None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Clear screen and set title
    if cfg!(windows) {
        let _ = std::process::Command::new("cmd").args(["/c", "cls"]).status();
        let _ = std::process::Command::new("cmd").args(["/c", "title", "PulseNet - IP Discovery Tool"]).status();
    } else {
        print!("\x1B]0;PulseNet - IP Discovery Tool\x07");
        print!("\x1B[2J\x1B[1;1H");
    }

    let args = Args::parse();
    let scanner = Scanner::new(&args.ports, args.timeout);

    print_banner();
    print_config(&args);

    if args.generate_only {
        println!("{}", "[-] Mode: Only generating IPs...".yellow());
        for _ in 0..args.count {
            println!("{}", Scanner::generate_random_ip());
        }
        return Ok(());
    }

    let pb = ProgressBar::new(args.count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{elapsed_precise}] [{bar:40.magenta/blue}] {pos}/{len} ({eta}) | Found: {msg} | {per_sec}")?
            .progress_chars("━╾ "),
    );

    let mut found_count = 0;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.output)?;

    let mut stream = futures::stream::iter(0..args.count)
        .map(|_| {
            let ip = Scanner::generate_random_ip();
            let scanner = &scanner;
            async move { (ip, scanner.check_ip_details(ip).await) }
        })
        .buffer_unordered(args.workers);

    while let Some((ip, port_found)) = stream.next().await {
        if let Some(port) = port_found {
            found_count += 1;
            let timestamp = Local::now().format("%H:%M:%S").to_string();
            
            pb.set_message(found_count.to_string());
            pb.suspend(|| {
                println!(
                    "{} [{}] {} {} {}:{}{}",
                    "✔".green().bold(),
                    timestamp.bright_black(),
                    "ACTIVE".on_green().white().bold(),
                    "⋙".bright_black(),
                    ip.to_string().bright_white().bold(),
                    port.to_string().yellow(),
                    "".to_string()
                );
                
                if let Err(e) = writeln!(file, "{}", ip) {
                    eprintln!("Logging error: {}", e);
                }
            });
        }
        pb.inc(1);
    }

    pb.finish_with_message("FINISHED");
    print_summary(found_count, &args.output);

    Ok(())
}

fn print_banner() {
    let art = r#"
    ____        __          _   __     __ 
   / __ \__  __/ /____ ___ / | / /__  / /_
  / /_/ / / / / / ___/ _ \/  |/ / _ \/ __/
 / ____/ /_/ / (__  )  __/ /|  /  __/ /_  
/_/    \__,_/_/____/\___/_/ |_/\___/\__/  
    "#;
    println!("{}", art.bright_cyan().bold());
    println!("{} {} {}", ">>".bright_black(), "THE FASTEST IP DISCOVERY TOOL".bright_white().bold(), "v0.1.0".bright_black());
    println!();
}

fn print_config(args: &Args) {
    println!("{}", "┌──────── SETUP CONFIGURATION ────────┐".bright_black());
    println!(" {:<18} : {}", "Target Count".cyan(), args.count.to_string().yellow());
    println!(" {:<18} : {}ms", "Timeout".cyan(), args.timeout.to_string().yellow());
    println!(" {:<18} : {}", "Threads".cyan(), args.workers.to_string().yellow());
    println!(" {:<18} : {}", "Ports".cyan(), args.ports.to_string().yellow());
    println!(" {:<18} : {}", "Log File".cyan(), args.output.to_string().magenta());
    println!("{}\n", "└─────────────────────────────────────┘".bright_black());
}

fn print_summary(total: u32, file: &str) {
    println!("\n{}", "  ┌──────────────────────────────────────────────────┐".bright_black());
    println!("  │  {:^48}│", "COMPLETED".bright_green().bold());
    println!("{}", "  ├──────────────────────────────────────────────────┤".bright_black());
    println!("  │  {:<25} : {:<20}│", "Total Discoveries".white(), total.to_string().yellow().bold());
    println!("  │  {:<25} : {:<20}│", "Logs Saved To".white(), file.magenta().italic());
    println!("  │  {:<25} : {:<20}│", "Status".white(), "Success".bright_cyan());
    println!("{}", "  └──────────────────────────────────────────────────┘".bright_black());
    println!("          {}\n", "Thank you for using PulseNet!".bright_black().italic());
}
