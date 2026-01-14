# ⚡ PulseNet v1.0

PulseNet is an ultra-fast, professional-grade IP discovery tool designed for efficient mapping of large network ranges. It combines asynchronous performance, intelligent rate limiting, and precise analytics.

## ✨ Features

*   **Ultra-Performance:** Built on Rust's `tokio` and `futures` libraries for maximum concurrency.
*   **Smart Filtering:** Automatically skips private (RFC1918), loopback, link-local, and reserved network ranges.
*   **Flexible IP Sources:**
    *   **Random:** Discover active hosts across random public IPs.
    *   **CIDR:** Target specific network ranges (e.g., `1.2.3.0/24`).
    *   **File:** Load a custom list of IPs from a text file.
*   **Adaptive Control:** Built-in rate limiting (CPS) and adjustable worker counts prevent network saturation.
*   **Analytics:** Real-time tracking of average latency, timeouts, and connection errors.
*   **Configurability:** TOML-based configuration file and comprehensive CLI arguments.
*   **Modern Output:** CSV or JSON logging for easy post-processing (ELK, jq, Python).

## 🚀 Quick Start

### 1. Build the project
Ensure you have [Rust](https://rustup.rs/) installed.
```bash
cargo build --release
```

### 2. Run the tool

#### **Windows (PowerShell or CMD)**
On Windows, use backslashes or execute the `.exe` directly:
```powershell
# PowerShell
.\target\release\PulseNet.exe --count 5000

# Command Prompt (CMD)
target\release\PulseNet.exe --count 5000
```

#### **Linux / macOS (UNIX)**
On UNIX systems, use forward slashes:
```bash
./target/release/PulseNet --count 5000
```

#### **Using Cargo (Cross-platform)**
Alternatively, run directly with cargo:
```bash
cargo run --release -- --count 5000
```

## 🛠️ CLI Arguments

| Argument | Description | Default |
| :--- | :--- | :--- |
| `-c, --count` | Number of IPs to scan (random mode) | 1000 |
| `-w, --workers` | Maximum concurrent connections | 64 |
| `-r, --rate` | Maximum connections per second (CPS) | 500 |
| `-t, --timeout` | Timeout per IP (ms) | 1500 |
| `-p, --ports` | Ports to check (comma separated) | 80,443,22,8080 |
| `--cidr` | CIDR ranges to scan | - |
| `--json` | Output results in JSON format | False |
| `--simulate` | Dry run without network activity | False |
| `--quiet` | Minimal UI (ideal for automation/scripts) | False |

## 📁 Configuration (pulsenet.toml)

You can save your persistent settings in a `pulsenet.toml` file:

```toml
count = 10000
workers = 128
rate = 1000
ports = "80,443,8080,3306"
output = "global_scan.log"
json = true
```

## 📊 Logs

Results are saved to `pulse_results.log` by default.
**CSV Format (Default):** `Timestamp,IP,Port,Latency(ms)`
**JSON Format:** `{"timestamp":"...","ip":"...","port":80,"latency_ms":15}`

---
*Developed with a focus on performance and ethical security testing.*
