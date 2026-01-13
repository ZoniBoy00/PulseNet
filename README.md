# ⚡ PulseNet - Ultra-Fast IP Discovery Tool

PulseNet is a state-of-the-art, asynchronous IP discovery application written in Rust. Engineered for speed and precision, it allows you to scan the network for active nodes with extreme efficiency.

## ✨ Key Features

- **🚀 Performance**: Leverages Rust's `Tokio` runtime for non-blocking, asynchronous I/O.
- **🎨 Modern UI**: Beautiful terminal presence with ASCII art, real-time metrics, and formatted output.
- **📝 Clean Logging**: Saves results as a clean list of IP addresses in `pulse_results.log` for easy integration with other tools.
- **⚙️ Configurable**: Fully customizable through command-line arguments (count, workers, timeout, ports).
- **🛡️ Optimized**: Intelligent worker management to maintain high speed without saturating your network.

## 🚀 Getting Started

### 1. Prerequisites
Make sure you have [Rust](https://rustup.rs/) installed on your system. You can verify it by running:
```bash
rustc --version
```

### 2. Installation
Clone the repository or download the source code, then navigate to the project folder:
```bash
cd PulseNet
```

### 3. Running with Cargo (Recommended)
The easiest way to run PulseNet is using `cargo run`. This automatically compiles and executes the tool.

**For maximum speed (optimised build):**
```bash
cargo run --release -- [arguments]
```
*Note: Use `--` to separate cargo arguments from PulseNet arguments.*

### 4. Compiling and Executing Binary
If you want to compile the standalone executable:
```bash
# Compile
cargo build --release

# Run on Windows
target\release\PulseNet.exe [arguments]

# Run on Linux/macOS
./target/release/PulseNet [arguments]
```

## 🛠️ Command-Line Arguments

PulseNet is highly flexible via CLI flags:

| Short | Long | Description | Default |
|-------|------|-------------|---------|
| `-c` | `--count` | Number of random IPs to check | 1000 |
| `-w` | `--workers` | Concurrent scan threads | 64 |
| `-p` | `--ports` | Ports to scan (comma separated) | 80,443,22,8080 |
| `-t` | `--timeout` | Connection timeout in ms | 800 |
| `-o` | `--output` | Where to save found IPs | pulse_results.log |
| `-g` | `--generate-only` | Generate IPs without scanning | false |

### Usage Examples

**Quick Scan (1000 IPs):**
```bash
cargo run --release
```

**Custom Scan (5000 IPs, Port 80 & 443):**
```bash
cargo run --release -- -c 5000 -p "80,443"
```

**Aggressive Scan (10000 IPs, 128 threads):**
```bash
cargo run --release -- -c 10000 -w 128 -t 500
```

## 📊 Output Format

The `pulse_results.log` file will contain a clean, newline-delimited list of active IP addresses, perfect for piping into other tools:
```text
104.26.10.228
172.67.74.152
8.8.8.8
```

## 📜 Disclaimer

This tool is intended for educational purposes and authorized network security testing only. Use it responsibly and always ensure you have permission before scanning any network.

---
**PulseNet** - *The fastest way to discover the pulse of the internet.*
