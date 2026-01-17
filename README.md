# NetSpeed-Lite

A lightweight ISP speed monitoring service written in Rust that uses the [Ookla Speedtest CLI](https://www.speedtest.net/apps/cli) to continuously monitor your internet connection, expose metrics via Prometheus, and send notifications. Perfect for tracking ISP performance, detecting slowdowns, and integrating with monitoring dashboards.

## Features

- **Automated Speed Testing**: Runs speed tests on a configurable schedule (hourly by default)
- **Prometheus Metrics**: Exposes download/upload speeds, latency, and jitter for monitoring
- **Flexible Scheduling**: Hourly aligned mode
- **Smart Notifications**: Sends alerts via [ntfy](https://ntfy.sh) (supports self-hosted instances)
- **HTTP API**: Provides `/metrics` and `/healthz` endpoints for monitoring
- **Lightweight**: Built with Rust for minimal resource usage
- **Docker Ready**: Multi-stage build with Ookla CLI pre-installed

> **Privacy Note**: This tool uses Ookla's Speedtest CLI, which collects and shares data with ISPs and manufacturers per their [privacy policy](https://www.speedtest.net/about/privacy). If privacy is a concern, consider self-hosting [LibreSpeed](https://github.com/librespeed/speedtest) as an alternative.

## Quick Start

### Using Docker Compose (Recommended)

```yaml
services:
  netspeed-lite:
    image: ghcr.io/yourusername/netspeed-lite:latest
    container_name: netspeed-lite
    env_file:
      - .env
    ports:
      - "9109:9109"
    restart: unless-stopped
```

Create a `.env` file:

```bash
cp .env.example .env
# Edit .env with your configuration
```

Start the service:

```bash
docker-compose up -d
```

### Using Docker

```bash
docker run -d \
  --name netspeed-lite \
  -p 9109:9109 \
  -e NETSPEED_TIMEZONE=Europe/London \
  -e NETSPEED_INTERVAL_HOURS=1 \
  -e NETSPEED_NTFY_URL=https://ntfy.sh/your-topic \
  ghcr.io/yourusername/netspeed-lite:latest
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/netspeed-lite.git
cd netspeed-lite

# Build with Cargo
cargo build --release

# Or use Make
make build

# Run
cp .env.example .env
# Edit .env with your configuration
./target/release/netspeed-lite
```

## Configuration

All configuration is done via environment variables. Copy `.env.example` to `.env` and customize:

| Variable | Required | Default | Description |
| -------- | -------- | ------- | ----------- |
| `NETSPEED_SCHEDULE_MODE` | No | `hourly_aligned` | Scheduling mode: `hourly_aligned`, `interval`, or `cron` |
| `NETSPEED_SCHEDULE` | If `cron` | - | Cron expression for scheduling (e.g., `0 */30 * * * *`) |
| `NETSPEED_INTERVAL_SECONDS` | If `interval` | `3600` | Run interval in seconds (default 1h) |
| `NETSPEED_TIMEZONE` | No | `Europe/London` | IANA timezone for scheduling (e.g., `America/New_York`, `Asia/Tokyo`) |
| `NETSPEED_BIND` | No | `0.0.0.0:9109` | HTTP server bind address |
| `NETSPEED_NTFY_URL` | No | - | ntfy topic URL for notifications (e.g., `https://ntfy.sh/your-topic`) |
| `NETSPEED_NTFY_TOKEN` | No | - | ntfy authentication token (optional) |
| `NETSPEED_NTFY_TITLE` | No | `netspeed-lite` | Notification title |
| `NETSPEED_NTFY_TAGS` | No | `speedtest,isp` | Comma-separated notification tags |
| `NETSPEED_NTFY_PRIORITY` | No | `3` | Notification priority (1-5) |
| `NETSPEED_NOTIFY_ON` | No | `success,failure` | When to notify: `success`, `failure`, or both |
| `NETSPEED_TIMEOUT_SECONDS` | No | `120` | Speedtest execution timeout |
| `RUST_LOG` | No | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |

### Scheduling Modes

NetSpeed-Lite supports three scheduling modes:

1. **Hourly Aligned (default)**
   - Runs exactly at the top of the hour (e.g., 10:00, 11:00)
   - Configuration: `NETSPEED_SCHEDULE_MODE=hourly_aligned`

2. **Interval**
   - Runs every X seconds
   - Configuration: `NETSPEED_SCHEDULE_MODE=interval`, `NETSPEED_INTERVAL_SECONDS=3600`

3. **Cron**
   - Uses standard cron expressions for flexible scheduling
   - Configuration: `NETSPEED_SCHEDULE_MODE=cron`, `NETSPEED_SCHEDULE="0 */30 * * * *"`
   - Use [crontab.guru](https://crontab.guru) to build expressions

- `0 * * * *` - Every hour at :00 (default)
- `*/30 * * * *` - Every 30 minutes
- `0 */6 * * *` - Every 6 hours
- `0 9,21 * * *` - At 9 AM and 9 PM daily
- `0 0 * * *` - Once daily at midnight
- `0 0 * * 0` - Once weekly on Sunday

**Cron syntax:** `minute hour day month weekday`

- `*` - Any value
- `*/N` - Every N units
- `N,M` - Specific values N and M
- `N-M` - Range from N to M

### Minimal Configuration

The simplest setup only requires setting your schedule and notification URL:

```bash
NETSPEED_SCHEDULE="0 0 * * * *"
NETSPEED_NTFY_URL=https://ntfy.sh/my-speedtest
```

## Prometheus Integration

### Metrics Endpoint

Access metrics at `http://localhost:9109/metrics`

### Available Metrics

| Metric | Type | Description |
| ------ | ---- | ----------- |
| `netspeed_last_success_timestamp` | Gauge | Unix timestamp of last successful test |
| `netspeed_runs_total` | Counter | Total number of speed tests (labeled by status) |
| `netspeed_duration_seconds` | Histogram | Test execution duration |
| `netspeed_download_bps` | Gauge | Download speed in bits per second |
| `netspeed_upload_bps` | Gauge | Upload speed in bits per second |
| `netspeed_latency_seconds` | Gauge | Latency in seconds |
| `netspeed_jitter_seconds` | Gauge | Jitter in seconds (if available) |
| `netspeed_notify_total` | Counter | Notification attempts (labeled by status) |
| `netspeed_process_cpu_usage` | Gauge | Process CPU usage percentage |
| `netspeed_process_memory_bytes` | Gauge | Process memory usage in bytes |

### Prometheus Configuration

Add to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'netspeed'
    static_configs:
      - targets: ['netspeed-lite:9109']
    scrape_interval: 60s
```

### Grafana Dashboard

Create panels to visualize:

- **Download/Upload Speed Over Time**: Line graph of `netspeed_download_bps` and `netspeed_upload_bps`
- **Latency Trends**: Line graph of `netspeed_latency_seconds`
- **Test Success Rate**: `rate(netspeed_runs_total{status="success"}[1h])`
- **Current Speed**: Stat panel showing latest values

**Example PromQL Queries:**

```promql
# Download speed in Mbps
netspeed_download_bps / 1000000

# Upload speed in Mbps
netspeed_upload_bps / 1000000

# Latency in milliseconds
netspeed_latency_seconds * 1000

# Test success rate (last hour)
rate(netspeed_runs_total{status="success"}[1h])
```

## Notifications

When configured with `NETSPEED_NTFY_URL`, the service sends notifications for test results.

### Success Notification

```text
⬇️ Download: 812.3 Mbps
⬆️ Upload: 42.1 Mbps
📡 Ping: 18.4 ms
⏱️ Duration: 52s
📊 Jitter: 2.1 ms
```

### Failure Notification

```text
❌ Speed test failed
timeout after 120s
```

### Notification Options

Control when notifications are sent:

```bash
# Only on success
NETSPEED_NOTIFY_ON=success

# Only on failure
NETSPEED_NOTIFY_ON=failure

# Both (default)
NETSPEED_NOTIFY_ON=success,failure
```

## API Endpoints

### GET /

Simple web UI with links to metrics and health check endpoints.

### GET /metrics

Prometheus metrics in text exposition format.

**Example:**

```bash
curl http://localhost:9109/metrics
```

### GET /healthz

Health check endpoint that returns service status.

**Response:**

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

## Use Cases

- **ISP Performance Tracking**: Monitor your internet speed over time to hold your ISP accountable
- **Network Troubleshooting**: Identify patterns in slowdowns or outages
- **SLA Verification**: Ensure you're getting the speeds you're paying for
- **Homelab Monitoring**: Track network performance alongside other infrastructure metrics
- **Automated Alerts**: Get notified immediately when speeds drop below expectations

## Architecture

```text
┌─────────────┐
│  Scheduler  │──┐
└─────────────┘  │
                 │  Triggers
                 ▼
┌─────────────┐     ┌──────────────┐
│   Runner    │────▶│   Metrics    │
└─────────────┘     └──────────────┘
       │                    │
       │                    │
       ▼                    ▼
┌─────────────┐     ┌──────────────┐
│  Notifier   │     │ HTTP Server  │
└─────────────┘     └──────────────┘
       │                    │
       ▼                    ▼
   ntfy.sh            Prometheus
```

## Troubleshooting

### Speedtest CLI not found

Ensure the Ookla Speedtest CLI is installed and available in PATH:

```bash
speedtest --version
```

For Docker users, the CLI is pre-installed in the image.

### No metrics appearing

Check that:

1. The service is running: `curl http://localhost:9109/healthz`
2. Metrics endpoint is accessible: `curl http://localhost:9109/metrics`
3. At least one test has completed (check logs: `docker logs netspeed-lite`)

### Notifications not working

Verify:

1. `NETSPEED_NTFY_URL` is set correctly
2. The ntfy topic is accessible: `curl -d "test" https://ntfy.sh/your-topic`
3. Check logs for notification errors
4. Ensure `NETSPEED_NOTIFY_ON` includes the desired event types

### Tests timing out

If tests consistently timeout:

1. Increase timeout: `NETSPEED_TIMEOUT_SECONDS=180`
2. Check your internet connection
3. Try running speedtest manually: `speedtest --format=json`

## Development

### Prerequisites

- Rust 1.75 or later
- Docker (for container testing)
- Ookla Speedtest CLI (for local testing)

### Local Development

```bash
# Clone and build
git clone https://github.com/yourusername/netspeed-lite.git
cd netspeed-lite
cargo build

# Run tests
cargo test

# Run locally
cp .env.example .env
cargo run
```

### Using Make

```bash
# Show all available commands
make help

# Run all CI checks locally
make ci

# Development workflow
make dev

# Build and run Docker
make docker-build
make docker-run
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Development workflow and branching strategy
- Pull request guidelines
- Code style and testing standards
- Local development setup

## License

MIT

## Acknowledgments

- [Ookla Speedtest CLI](https://www.speedtest.net/apps/cli) - Industry-standard speed testing
- [ntfy](https://ntfy.sh) - Simple notification service
- [Axum](https://github.com/tokio-rs/axum) - Ergonomic web framework
- [Antigravity](https://deepmind.google/technologies/antigravity/) - Developed with the assistance of Google DeepMind's agentic coding assistant
