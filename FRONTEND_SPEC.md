# HFT-7ZERO x HYPERION TRADE: Frontend POC Specification

This document outlines the UI elements and data requirements for the Proof of Concept (POC) dashboard. The goal is to visualize the sub-millisecond decision-making process of the Stealth Execution system.

## 1. Dashboard Architecture
- **Tech Stack Recommendation:** React.js or Vue.js with **TailwindCSS**.
- **Visualization:** D3.js or ECharts (Canvas-based for high-frequency updates).
- **Communication:** WebSockets (port 8080) for real-time telemetry.

## 2. Core Components

### A. The "Confluence Matrix" (EV-ATR Sizing)
Visualizes how the system determines position size $.
- **Inputs:**
  - {Kelly}$ (Profitability Bias)
  - {Vol}$ (Volatility Dampening)
  - {Conf}$ (Signal Strength)
- **UI Element:** A 3-dial Gauge or a Spider Chart.
- **Centerpiece:** Large digital readout of Current $ (Lots/Units).

### B. Schur Routing Topology
Displays the optimal fragmentation of orders across venues.
- **UI Element:** Dynamic Horizontal Stacked Bar Chart.
- **Labels:** Venue ID (e.g., CME, LMAX, ICE).
- **Status Lights:**
  - **Adelic Valid:** Green/Red LED.
  - **Blow-up Detection:** Flash Red if triggered.
- **Metrics:** Real-time Cost Savings (Estimated vs. Execution).

### C. Risk Gate Telemetry (The Lambda Grid)
Real-time status of safety triggers.
- **UI Element:** 2x3 Grid of tiles labeled $\lambda_1$ to $\lambda_6$.
- **States:**
  - **Dimmed:** Inactive.
  - **Blue Glow:** Evaluating.
  - **Amber/Red:** Triggered (Gate Closed).
- **System Status Indicator:** Massive header showing "OPEN", "LOCKED", or "SHUTDOWN".

### D. Latency "1ms Wall" Monitor
Critical performance tracking.
- **UI Element:** High-speed Sparkline with a fixed red horizontal baseline at 1,000,000ns.
- **Readouts:** P50, P99, and Max Latency.
- **Alert:** Screen perimeter flashes white if a latency spike occurs.

### E. Stealth Detection Probability
Visualizes the "Stealth" effectiveness.
- **UI Element:** Area Chart showing "Detection Probability" (0% to 100%).
- **Goal:** Keep the line below the 0.1% threshold.

## 3. Real-Time Data Schema (JSON)
The backend will emit frames like this:
```json
{
  "timestamp": 1713184000000000,
  "system_status": "OPEN",
  "sizing": {
    "q_t": 26012.8,
    "f_kelly": 50.66,
    "g_vol": 0.79,
    "h_conf": 0.65
  },
  "routing": {
    "venues": [
      {"id": "CME", "weight": 0.65, "qty": 16908},
      {"id": "LMAX", "weight": 0.35, "qty": 9104}
    ],
    "adelic_valid": true
  },
  "risk": {
    "triggers": ["lambda1"],
    "volatility_regime": 2
  },
  "performance": {
    "latency_ns": 420150,
    "stealth_score": 0.9998
  }
}
```

## 4. Interaction Points (POC Controls)
- **Panic Button:** Large red button to trigger `Emergency Shutdown`.
- **Regime Override:** Toggle to manually switch between Low/Medium/High volatility profiles.
- **Dry Run Toggle:** Safety switch for simulation vs. live mode.

## 5. Deployment Instructions for POC
1.  **Clone Repo:** Frontend should live in a `dashboard/` subdirectory.
2.  **Mock Server:** Use `json-server` or a simple Python script to broadcast the JSON schema above if the Rust backend is not yet streaming.
3.  **Handoff:** Deliver the UI as a static build that can be served via the Rust `os/dashboard.rs` module.
