use anyhow::Result;
use chrono::Local;
use clap::Parser;
use colored::*;
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use indicatif::{ProgressBar, ProgressStyle};
use ipnet::Ipv4Net;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;

#[derive(Parser, Debug, Serialize, Deserialize, Clone)]
#[command(author, version = "0.2.0", about = "⚡ PulseNet - Professional IP Discovery Tool")]
struct Args {
    /// Total number of IPs to scan (for random mode)
    #[arg(short, long, default_value_t = 1000)]
    count: u32,

    /// Timeout for each IP check in milliseconds
    #[arg(short, long, default_value_t = 1500)]
    timeout: u64,

    /// Number of concurrent workers (max concurrent connections)
    #[arg(short = 'w', long, default_value_t = 64)]
    workers: usize,

    /// Max connections per second (rate limiting)
    #[arg(short = 'r', long, default_value_t = 500)]
    rate: u32,

    /// Ports to check (comma separated)
    #[arg(short, long, default_value = "80,443,22,8080")]
    ports: String,

    /// Output file for logs
    #[arg(short, long, default_value = "pulse_results.log")]
    output: String,

    /// CIDR ranges to scan (comma separated)
    #[arg(short, long)]
    cidr: Option<String>,

    /// File containing list of IPs to scan
    #[arg(short, long)]
    file: Option<String>,

    /// Dry run without network activity
    #[arg(short, long)]
    simulate: bool,

    /// Output results in JSON format
    #[arg(short, long)]
    json: bool,

    /// Quiet mode (no UI, minimal logs)
    #[arg(short, long)]
    quiet: bool,

    /// Config file path (TOML)
    #[arg(short, long, default_value = "pulsenet.toml")]
    config: String,
}

#[derive(Serialize)]
struct ScanResult {
    timestamp: String,
    ip: String,
    port: u16,
    latency_ms: u128,
}

// --- Logic Modules ---

mod filter {
    use std::net::Ipv4Addr;
    pub fn is_public_ipv4(ip: Ipv4Addr) -> bool {
        let octets = ip.octets();
        match octets[0] {
            10 => false,
            100 if (64..=127).contains(&octets[1]) => false, // CGNAT
            127 => false,
            169 if octets[1] == 254 => false,
            172 if (16..=31).contains(&octets[1]) => false,
            192 if octets[1] == 168 => false,
            192 if octets[1] == 0 && octets[2] == 2 => false, // Documentation
            198 if octets[1] == 51 && octets[2] == 100 => false,
            203 if octets[1] == 0 && octets[2] == 113 => false,
            o if o >= 224 => false, // Multicast & Reserved
            _ => true,
        }
    }
}

trait IpSource: Send {
    fn next_ip(&mut self) -> Option<Ipv4Addr>;
    fn total_count(&self) -> usize;
}

struct RandomSource { count: usize, current: usize }
impl IpSource for RandomSource {
    fn next_ip(&mut self) -> Option<Ipv4Addr> {
        if self.current >= self.count { return None; }
        self.current += 1;
        let mut rng = rand::thread_rng();
        loop {
            let ip = Ipv4Addr::new(
                rng.gen_range(0..=255),
                rng.gen_range(0..=255),
                rng.gen_range(0..=255),
                rng.gen_range(0..=255)
            );
            if filter::is_public_ipv4(ip) { return Some(ip); }
        }
    }
    fn total_count(&self) -> usize { self.count }
}

struct MultiIpSource { ips: Vec<Ipv4Addr> }
impl MultiIpSource {
    fn from_cidr(cidr_strs: &str) -> Self {
        let mut ips = Vec::new();
        for s in cidr_strs.split(',') {
            if let Ok(net) = s.trim().parse::<Ipv4Net>() {
                ips.extend(net.hosts());
            }
        }
        let mut rng = rand::thread_rng();
        ips.shuffle(&mut rng);
        Self { ips }
    }
    fn from_file(path: &str) -> Self {
        let mut ips = Vec::new();
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(ip) = line.trim().parse::<Ipv4Addr>() { ips.push(ip); }
            }
        }
        let mut rng = rand::thread_rng();
        ips.shuffle(&mut rng);
        Self { ips }
    }
}
impl IpSource for MultiIpSource {
    fn next_ip(&mut self) -> Option<Ipv4Addr> { self.ips.pop() }
    fn total_count(&self) -> usize { self.ips.len() }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
enum ScanError { Timeout, ConnectionRefused, Unreachable }

struct Scanner {
    ports: Vec<u16>,
    timeout_ms: u64,
    simulate: bool,
}

impl Scanner {
    fn new(args: &Args) -> Self {
        let ports = args.ports.split(',').filter_map(|s| s.trim().parse::<u16>().ok()).collect();
        Self { ports, timeout_ms: args.timeout, simulate: args.simulate }
    }

    async fn check_ip(&self, ip: Ipv4Addr) -> (Option<u16>, Option<u128>, Option<ScanError>) {
        if self.simulate {
            let mut rng = rand::thread_rng();
            tokio::time::sleep(Duration::from_millis(rng.gen_range(10..100))).await;
            return if rng.gen_bool(0.05) { (Some(self.ports[0]), Some(rng.gen_range(5..50)), None) } 
                   else { (None, None, Some(ScanError::Timeout)) };
        }

        let start = std::time::Instant::now();
        let port_timeout = Duration::from_millis(self.timeout_ms / self.ports.len().max(1) as u64);
        let mut last_error = None;

        for &port in &self.ports {
            let addr = SocketAddr::new(ip.into(), port);
            match timeout(port_timeout, TcpStream::connect(addr)).await {
                Ok(Ok(_)) => return (Some(port), Some(start.elapsed().as_millis()), None),
                Ok(Err(e)) => {
                    last_error = Some(match e.kind() {
                        std::io::ErrorKind::ConnectionRefused => ScanError::ConnectionRefused,
                        _ => ScanError::Unreachable,
                    });
                }
                Err(_) => { if last_error.is_none() { last_error = Some(ScanError::Timeout); } }
            }
        }
        (None, None, last_error)
    }
}

// --- Main Engine ---

#[derive(Default)]
struct Stats {
    found: u32,
    timeouts: u32,
    refused: u32,
    unreachable: u32,
    total_processed: u32,
    total_latency: u128,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = Args::parse();
    if Path::new(&args.config).exists() {
        if let Ok(content) = std::fs::read_to_string(&args.config) {
            if let Ok(config) = toml::from_str::<Args>(&content) {
                args = config; // Basic merge: file overrides CLI defaults if used, but CLI still wins if passed
            }
        }
    }

    if !args.quiet { setup_terminal(); }

    let scanner = Arc::new(Scanner::new(&args));
    let mut source: Box<dyn IpSource> = if let Some(cidr) = &args.cidr {
        Box::new(MultiIpSource::from_cidr(cidr))
    } else if let Some(file_path) = &args.file {
        Box::new(MultiIpSource::from_file(file_path))
    } else {
        Box::new(RandomSource { count: args.count as usize, current: 0 })
    };

    let total = source.total_count();
    if !args.quiet { 
        print_banner();
        print_config(&args, total);
    }

    let pb = if !args.quiet {
        let p = ProgressBar::new(total as u64);
        p.set_style(ProgressStyle::default_bar()
            .template("{spinner:.cyan} [{elapsed_precise}] [{bar:40.magenta/blue}] {pos}/{len} | Hits: {msg} | {per_sec}")?
            .progress_chars("━╾ "));
        Some(p)
    } else { None };

    let mut stats = Stats::default();
    let mut file = OpenOptions::new().create(true).append(true).open(&args.output)?;
    let mut clean_file = OpenOptions::new().create(true).append(true).open("found_ips.txt")?;

    let mut ips = Vec::with_capacity(total.min(100_000));
    while let Some(ip) = source.next_ip() { ips.push(ip); }

    let limiter = Arc::new(RateLimiter::direct(Quota::per_second(NonZeroU32::new(args.rate).unwrap())));
    let semaphore = Arc::new(Semaphore::new(args.workers));

    let mut stream = futures::stream::iter(ips)
        .map(|ip| {
            let sc = Arc::clone(&scanner);
            let lim = Arc::clone(&limiter);
            let sem = Arc::clone(&semaphore);
            async move {
                lim.until_ready().await;
                let _permit = sem.acquire().await.unwrap();
                (ip, sc.check_ip(ip).await)
            }
        })
        .buffer_unordered(2048);

    while let Some((ip, (port_found, latency, error))) = stream.next().await {
        stats.total_processed += 1;
        if let Some(port) = port_found {
            stats.found += 1;
            let lat = latency.unwrap_or(0);
            stats.total_latency += lat;
            
            let ts_full = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            if let Some(ref p) = pb {
                p.set_message(stats.found.to_string());
                p.suspend(|| {
                    println!("{} [{}] {} {}:{} {}ms", "✔".green(), Local::now().format("%H:%M:%S").to_string().bright_black(), "ACTIVE".on_green().white().bold(), ip.to_string().bright_white().bold(), port.to_string().yellow(), lat.to_string().cyan());
                });
            }

            if !args.simulate {
                // Write to clean IP list
                let _ = writeln!(clean_file, "{}", ip);

                if args.json {
                    let res = ScanResult { timestamp: ts_full, ip: ip.to_string(), port, latency_ms: lat };
                    let _ = writeln!(file, "{}", serde_json::to_string(&res)?);
                } else {
                    let _ = writeln!(file, "[{}] {}, Port: {}, Latency: {}ms", ts_full, ip, port, lat);
                }
            }
        } else {
            match error {
                Some(ScanError::Timeout) => stats.timeouts += 1,
                Some(ScanError::ConnectionRefused) => stats.refused += 1,
                Some(ScanError::Unreachable) => stats.unreachable += 1,
                None => {}
            }
        }
        if let Some(ref p) = pb { p.inc(1); }
    }

    if let Some(p) = pb { p.finish_with_message("DONE"); }
    if !args.quiet { print_summary(&stats, &args.output, "found_ips.txt"); }
    Ok(())
}

// --- UI Helpers ---

fn setup_terminal() {
    if cfg!(windows) {
        let _ = std::process::Command::new("cmd").args(["/c", "cls"]).status();
        let _ = std::process::Command::new("cmd").args(["/c", "title", "PulseNet v1.0"]).status();
    } else {
        print!("\x1B]0;PulseNet v1.0\x07\x1B[2J\x1B[1;1H");
    }
}

fn print_banner() {
    println!("{}", r#"
    ____        __          _   __     __ 
   / __ \__  __/ /____ ___ / | / /__  / /_
  / /_/ / / / / / ___/ _ \/  |/ / _ \/ __/
 / ____/ /_/ / (__  )  __/ /|  /  __/ /_  
/_/    \__,_/_/____/\___/_/ |_/\___/\__/ v0.2
    "#.bright_cyan().bold());
}

fn print_config(args: &Args, total: usize) {
    println!("{}", "  ┌─────────────────────────────────────┐".bright_black());
    println!("  │ {:^35} │", "SCAN CONFIGURATION".bright_white().bold());
    println!("{}", "  ├─────────────────────────────────────┤".bright_black());
    println!("  │ {:<15} : {:<17} │", "Targets".cyan(), total.to_string().yellow());
    println!("  │ {:<15} : {:<17} │", "Timeout".cyan(), format!("{}ms", args.timeout).yellow());
    println!("  │ {:<15} : {:<17} │", "Rate Limit".cyan(), format!("{}/s", args.rate).yellow());
    println!("  │ {:<15} : {:<17} │", "Workers".cyan(), args.workers.to_string().yellow());
    println!("  │ {:<15} : {:<17} │", "Ports".cyan(), args.ports.to_string().yellow());
    println!("{}", "  └─────────────────────────────────────┘".bright_black());
    println!();
}

fn print_summary(stats: &Stats, log_file: &str, clean_file: &str) {
    let avg = if stats.found > 0 { stats.total_latency / stats.found as u128 } else { 0 };
    
    println!("\n{}", "  ┌─────────────────────────────────────┐".bright_black());
    println!("  │ {:^35} │", "SCAN COMPLETED".bright_green().bold());
    println!("{}", "  ├─────────────────────────────────────┤".bright_black());
    println!("  │ {:<15} : {:<17} │", "Total Hits".white(), stats.found.to_string().green().bold());
    println!("  │ {:<15} : {:<17} │", "Avg Latency".white(), format!("{}ms", avg).cyan());
    println!("  │ {:<15} : {:<17} │", "Timeouts".white(), stats.timeouts.to_string().yellow());
    println!("  │ {:<15} : {:<17} │", "Refused".white(), stats.refused.to_string().red());
    println!("  │ {:<15} : {:<17} │", "Unreachable".white(), stats.unreachable.to_string().bright_black());
    println!("{}", "  ├─────────────────────────────────────┤".bright_black());
    println!("  │ {:<15} : {:<17} │", "Full Logs".white(), log_file.magenta().italic());
    println!("  │ {:<15} : {:<17} │", "Clean IPs".white(), clean_file.bright_white().italic());
    println!("{}", "  └─────────────────────────────────────┘".bright_black());
    println!("          {}\n", "Thank you for using PulseNet!".bright_black().italic());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_public_filter() {
        assert!(!filter::is_public_ipv4(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(filter::is_public_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
    }
    #[test]
    fn test_random_source() {
        let mut source = RandomSource { count: 5, current: 0 };
        assert!(source.next_ip().is_some());
    }
}
