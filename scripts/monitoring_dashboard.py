#!/usr/bin/env python3
# ============================================================
# MONITORING DASHBOARD
# ============================================================
# Real-time dashboard for HFT stealth system
# Displays latency, throughput, risk metrics
# Web interface with WebSocket updates
# ============================================================

import asyncio
import json
import time
import argparse
from datetime import datetime, timedelta
from collections import deque
from typing import Dict, List, Any

import aiohttp
import aiohttp.web
import plotly.graph_objs as go
import plotly.express as px
from plotly.subplots import make_subplots
import pandas as pd
import numpy as np

# ============================================================
# CONFIGURATION
# ============================================================

METRICS_ENDPOINT = "http://localhost:9090/metrics"
WEBSOCKET_PORT = 8765
HTTP_PORT = 8080
HISTORY_SECONDS = 300  # 5 minutes history
UPDATE_INTERVAL_MS = 100  # 100ms updates

# ============================================================
# METRICS COLLECTOR
# ============================================================

class MetricsCollector:
    def __init__(self):
        self.latencies = deque(maxlen=10000)
        self.throughput = deque(maxlen=1000)
        self.risk_scores = deque(maxlen=1000)
        self.detection_events = deque(maxlen=1000)
        self.timestamps = deque(maxlen=1000)
        
    async def fetch_metrics(self) -> Dict[str, Any]:
        """Fetch metrics from Prometheus endpoint"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(METRICS_ENDPOINT) as resp:
                    if resp.status == 200:
                        text = await resp.text()
                        return self.parse_metrics(text)
        except Exception as e:
            print(f"Error fetching metrics: {e}")
        
        return {}
    
    def parse_metrics(self, text: str) -> Dict[str, Any]:
        """Parse Prometheus metrics format"""
        metrics = {}
        
        for line in text.split('\n'):
            if line.startswith('#') or not line.strip():
                continue
            
            if ' ' in line:
                parts = line.split(' ')
                name = parts[0]
                try:
                    value = float(parts[1])
                    metrics[name] = value
                except ValueError:
                    pass
        
        # Extract key metrics
        return {
            'latency_p50_ns': metrics.get('hft_latency_p50_ns', 0),
            'latency_p95_ns': metrics.get('hft_latency_p95_ns', 0),
            'latency_p99_ns': metrics.get('hft_latency_p99_ns', 0),
            'latency_p999_ns': metrics.get('hft_latency_p999_ns', 0),
            'ticks_per_second': metrics.get('hft_ticks_per_second', 0),
            'orders_per_second': metrics.get('hft_orders_per_second', 0),
            'detection_probability': metrics.get('hft_detection_probability', 0),
            'total_pnl': metrics.get('hft_total_pnl', 0),
            'sharpe_ratio': metrics.get('hft_sharpe_ratio', 0),
            'cpu_percent': metrics.get('hft_cpu_percent', 0),
            'memory_mb': metrics.get('hft_memory_mb', 0),
        }
    
    def update_history(self, metrics: Dict[str, Any]):
        """Update historical data"""
        timestamp = time.time()
        self.timestamps.append(timestamp)
        self.latencies.append(metrics.get('latency_p99_ns', 0))
        self.throughput.append(metrics.get('ticks_per_second', 0))
        self.risk_scores.append(metrics.get('detection_probability', 0))
    
    def get_history_data(self) -> Dict[str, List]:
        """Get historical data for plotting"""
        timestamps = list(self.timestamps)
        return {
            'timestamps': timestamps,
            'latencies': list(self.latencies),
            'throughput': list(self.throughput),
            'risk_scores': list(self.risk_scores),
        }

# ============================================================
# DASHBOARD GENERATOR
# ============================================================

class DashboardGenerator:
    def __init__(self):
        self.collector = MetricsCollector()
        self.clients = set()
    
    def generate_html(self) -> str:
        """Generate dashboard HTML"""
        return f'''
        <!DOCTYPE html>
        <html>
        <head>
            <title>HFT Stealth System Dashboard</title>
            <style>
                body {{
                    font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
                    margin: 0;
                    padding: 20px;
                    background: #1a1a2e;
                    color: #eee;
                }}
                .dashboard {{
                    max-width: 1400px;
                    margin: 0 auto;
                }}
                .header {{
                    text-align: center;
                    margin-bottom: 30px;
                }}
                .header h1 {{
                    color: #00d4ff;
                    margin: 0;
                }}
                .header p {{
                    color: #888;
                    margin: 5px 0 0;
                }}
                .metrics-grid {{
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                    gap: 20px;
                    margin-bottom: 30px;
                }}
                .metric-card {{
                    background: #16213e;
                    border-radius: 10px;
                    padding: 20px;
                    text-align: center;
                    box-shadow: 0 4px 6px rgba(0,0,0,0.3);
                }}
                .metric-card h3 {{
                    margin: 0 0 10px;
                    font-size: 14px;
                    color: #888;
                    text-transform: uppercase;
                }}
                .metric-card .value {{
                    font-size: 32px;
                    font-weight: bold;
                    color: #00d4ff;
                }}
                .metric-card .unit {{
                    font-size: 12px;
                    color: #666;
                    margin-left: 5px;
                }}
                .metric-card .trend {{
                    font-size: 12px;
                    margin-top: 10px;
                }}
                .trend.up {{ color: #00ff88; }}
                .trend.down {{ color: #ff4444; }}
                .chart-container {{
                    background: #16213e;
                    border-radius: 10px;
                    padding: 20px;
                    margin-bottom: 20px;
                }}
                .status {{
                    display: inline-block;
                    width: 10px;
                    height: 10px;
                    border-radius: 50%;
                    margin-right: 8px;
                }}
                .status.healthy {{ background: #00ff88; }}
                .status.warning {{ background: #ffaa00; }}
                .status.critical {{ background: #ff4444; }}
                .footer {{
                    text-align: center;
                    padding: 20px;
                    color: #666;
                    font-size: 12px;
                }}
            </style>
            <script src="https://cdn.plot.ly/plotly-latest.min.js"></script>
        </head>
        <body>
            <div class="dashboard">
                <div class="header">
                    <h1>🔒 HFT Stealth System</h1>
                    <p>Real-time Monitoring Dashboard</p>
                </div>
                
                <div class="metrics-grid" id="metrics-grid">
                    <!-- Metrics will be populated here -->
                </div>
                
                <div class="chart-container">
                    <div id="latency-chart" style="height: 400px;"></div>
                </div>
                
                <div class="chart-container">
                    <div id="throughput-chart" style="height: 400px;"></div>
                </div>
                
                <div class="chart-container">
                    <div id="risk-chart" style="height: 400px;"></div>
                </div>
                
                <div class="footer">
                    <span class="status" id="health-status"></span>
                    <span id="health-text">System Health</span>
                    &nbsp;&nbsp;|&nbsp;&nbsp;
                    Last update: <span id="last-update">-</span>
                </div>
            </div>
            
            <script>
                let ws = null;
                
                function connectWebSocket() {{
                    ws = new WebSocket(`ws://${{window.location.host}}/ws`);
                    
                    ws.onmessage = function(event) {{
                        const data = JSON.parse(event.data);
                        updateDashboard(data);
                    }};
                    
                    ws.onclose = function() {{
                        setTimeout(connectWebSocket, 1000);
                    }};
                }}
                
                function updateDashboard(data) {{
                    // Update metric cards
                    const grid = document.getElementById('metrics-grid');
                    const metrics = [
                        {{ name: 'P99 Latency', value: (data.latency_p99_ns / 1000).toFixed(2), unit: 'μs', trend: data.latency_trend }},
                        {{ name: 'Throughput', value: data.ticks_per_second.toFixed(0), unit: 'ticks/s', trend: data.throughput_trend }},
                        {{ name: 'Detection Risk', value: (data.detection_probability * 100).toFixed(3), unit: '%', trend: -data.detection_trend }},
                        {{ name: 'Total P&L', value: data.total_pnl.toFixed(2), unit: '$', trend: data.pnl_trend }},
                        {{ name: 'Sharpe Ratio', value: data.sharpe_ratio.toFixed(2), unit: '', trend: 0 }},
                        {{ name: 'CPU Usage', value: data.cpu_percent.toFixed(1), unit: '%', trend: 0 }},
                        {{ name: 'Memory', value: data.memory_mb.toFixed(0), unit: 'MB', trend: 0 }},
                        {{ name: 'Stealth Score', value: ((1 - data.detection_probability) * 100).toFixed(1), unit: '%', trend: -data.detection_trend }}
                    ];
                    
                    let html = '';
                    for (const m of metrics) {{
                        const trendClass = m.trend > 0 ? 'up' : (m.trend < 0 ? 'down' : '');
                        const trendSymbol = m.trend > 0 ? '▲' : (m.trend < 0 ? '▼' : '');
                        html += `
                            <div class="metric-card">
                                <h3>${{m.name}}</h3>
                                <div class="value">${{m.value}}<span class="unit">${{m.unit}}</span></div>
                                <div class="trend ${{trendClass}}">${{trendSymbol}} ${{Math.abs(m.trend).toFixed(2)}}%</div>
                            </div>
                        `;
                    }}
                    grid.innerHTML = html;
                    
                    // Update charts
                    updateLatencyChart(data);
                    updateThroughputChart(data);
                    updateRiskChart(data);
                    
                    // Update health status
                    const healthStatus = document.getElementById('health-status');
                    const healthText = document.getElementById('health-text');
                    if (data.detection_probability > 0.05) {{
                        healthStatus.className = 'status critical';
                        healthText.textContent = '⚠️ HIGH DETECTION RISK';
                    }} else if (data.latency_p99_ns > 1000000) {{
                        healthStatus.className = 'status warning';
                        healthText.textContent = '⚠️ HIGH LATENCY';
                    }} else {{
                        healthStatus.className = 'status healthy';
                        healthText.textContent = '✓ SYSTEM HEALTHY';
                    }}
                    
                    document.getElementById('last-update').textContent = new Date().toLocaleTimeString();
                }}
                
                let latencyChart = null;
                let throughputChart = null;
                let riskChart = null;
                
                function updateLatencyChart(data) {{
                    const trace = {{
                        x: data.timestamps,
                        y: data.latencies,
                        type: 'scatter',
                        mode: 'lines',
                        name: 'P99 Latency',
                        line: {{ color: '#00d4ff', width: 2 }},
                        fill: 'tozeroy',
                        fillcolor: 'rgba(0, 212, 255, 0.1)'
                    }};
                    
                    const layout = {{
                        title: 'Latency (P99)',
                        xaxis: {{ title: 'Time', gridcolor: '#333' }},
                        yaxis: {{ title: 'Latency (μs)', gridcolor: '#333' }},
                        paper_bgcolor: '#16213e',
                        plot_bgcolor: '#16213e',
                        font: {{ color: '#eee' }}
                    }};
                    
                    if (latencyChart === null) {{
                        latencyChart = Plotly.newPlot('latency-chart', [trace], layout);
                    }} else {{
                        Plotly.react('latency-chart', [trace], layout);
                    }}
                }}
                
                function updateThroughputChart(data) {{
                    const trace = {{
                        x: data.timestamps,
                        y: data.throughput,
                        type: 'scatter',
                        mode: 'lines',
                        name: 'Throughput',
                        line: {{ color: '#00ff88', width: 2 }},
                        fill: 'tozeroy',
                        fillcolor: 'rgba(0, 255, 136, 0.1)'
                    }};
                    
                    const layout = {{
                        title: 'Throughput',
                        xaxis: {{ title: 'Time', gridcolor: '#333' }},
                        yaxis: {{ title: 'Ticks per Second', gridcolor: '#333' }},
                        paper_bgcolor: '#16213e',
                        plot_bgcolor: '#16213e',
                        font: {{ color: '#eee' }}
                    }};
                    
                    if (throughputChart === null) {{
                        throughputChart = Plotly.newPlot('throughput-chart', [trace], layout);
                    }} else {{
                        Plotly.react('throughput-chart', [trace], layout);
                    }}
                }}
                
                function updateRiskChart(data) {{
                    const trace = {{
                        x: data.timestamps,
                        y: data.risk_scores,
                        type: 'scatter',
                        mode: 'lines',
                        name: 'Detection Risk',
                        line: {{ color: '#ff4444', width: 2 }},
                        fill: 'tozeroy',
                        fillcolor: 'rgba(255, 68, 68, 0.1)'
                    }};
                    
                    const layout = {{
                        title: 'Detection Risk (ℙ ≈ 0 target)',
                        xaxis: {{ title: 'Time', gridcolor: '#333' }},
                        yaxis: {{ title: 'Probability', gridcolor: '#333', range: [0, 0.1] }},
                        paper_bgcolor: '#16213e',
                        plot_bgcolor: '#16213e',
                        font: {{ color: '#eee' }},
                        shapes: [{{
                            type: 'line',
                            y0: 0.001,
                            y1: 0.001,
                            x0: data.timestamps[0],
                            x1: data.timestamps[data.timestamps.length-1],
                            line: {{ color: '#00ff88', width: 2, dash: 'dash' }},
                            name: 'Target'
                        }}]
                    }};
                    
                    if (riskChart === null) {{
                        riskChart = Plotly.newPlot('risk-chart', [trace], layout);
                    }} else {{
                        Plotly.react('risk-chart', [trace], layout);
                    }}
                }}
                
                // Start WebSocket connection
                connectWebSocket();
            </script>
        </body>
        </html>
        '''

# ============================================================
# WEB SERVER
# ============================================================

class DashboardServer:
    def __init__(self):
        self.collector = MetricsCollector()
        self.generator = DashboardGenerator()
        self.websockets = set()
    
    async def handle_index(self, request):
        """Serve dashboard HTML"""
        html = self.generator.generate_html()
        return aiohttp.web.Response(text=html, content_type='text/html')
    
    async def handle_websocket(self, request):
        """Handle WebSocket connections for real-time updates"""
        ws = aiohttp.web.WebSocketResponse()
        await ws.prepare(request)
        
        self.websockets.add(ws)
        
        try:
            async for msg in ws:
                if msg.type == aiohttp.WSMsgType.CLOSE:
                    break
        finally:
            self.websockets.remove(ws)
        
        return ws
    
    async def broadcast_metrics(self):
        """Broadcast metrics to all connected clients"""
        while True:
            try:
                metrics = await self.collector.fetch_metrics()
                self.collector.update_history(metrics)
                history = self.collector.get_history_data()
                
                # Calculate trends
                data = {
                    **metrics,
                    'timestamps': history['timestamps'],
                    'latencies': [l / 1000 for l in history['latencies']],  # Convert to μs
                    'throughput': history['throughput'],
                    'risk_scores': history['risk_scores'],
                    'latency_trend': self.calculate_trend(history['latencies']),
                    'throughput_trend': self.calculate_trend(history['throughput']),
                    'detection_trend': self.calculate_trend(history['risk_scores']),
                    'pnl_trend': metrics.get('total_pnl', 0) > 0,
                }
                
                # Broadcast to all connected clients
                for ws in self.websockets.copy():
                    try:
                        await ws.send_json(data)
                    except Exception:
                        self.websockets.discard(ws)
                
                await asyncio.sleep(UPDATE_INTERVAL_MS / 1000)
                
            except Exception as e:
                print(f"Error broadcasting metrics: {e}")
                await asyncio.sleep(1)
    
    def calculate_trend(self, values: List[float]) -> float:
        """Calculate percentage trend over last 10 values"""
        if len(values) < 10:
            return 0
        
        recent = sum(values[-5:]) / 5
        older = sum(values[-10:-5]) / 5
        
        if older == 0:
            return 0
        
        return ((recent - older) / older) * 100
    
    async def run(self):
        """Run the dashboard server"""
        app = aiohttp.web.Application()
        app.router.add_get('/', self.handle_index)
        app.router.add_get('/ws', self.handle_websocket)
        
        # Start background task for metrics broadcasting
        asyncio.create_task(self.broadcast_metrics())
        
        # Run server
        runner = aiohttp.web.AppRunner(app)
        await runner.setup()
        site = aiohttp.web.TCPSite(runner, '0.0.0.0', HTTP_PORT)
        await site.start()
        
        print(f"Dashboard running at http://localhost:{HTTP_PORT}")
        print(f"WebSocket endpoint: ws://localhost:{WEBSOCKET_PORT}/ws")
        
        # Keep running
        await asyncio.Event().wait()

# ============================================================
# MAIN
# ============================================================

def main():
    parser = argparse.ArgumentParser(description='HFT Stealth Monitoring Dashboard')
    parser.add_argument('--port', type=int, default=HTTP_PORT, help='HTTP port')
    parser.add_argument('--metrics-url', type=str, default=METRICS_ENDPOINT, help='Metrics endpoint')
    
    args = parser.parse_args()
    
    # Update globals
    globals()['HTTP_PORT'] = args.port
    globals()['METRICS_ENDPOINT'] = args.metrics_url
    
    # Run dashboard
    dashboard = DashboardServer()
    asyncio.run(dashboard.run())

if __name__ == '__main__':
    main()
