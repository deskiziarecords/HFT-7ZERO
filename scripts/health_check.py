#!/usr/bin/env python3
"""Health check script for HFT Stealth System"""

import sys
import json
import argparse
from typing import Dict, Any

import aiohttp
import asyncio


class HealthChecker:
    def __init__(self, host: str = "localhost", port: int = 9090):
        self.base_url = f"http://{host}:{port}"
        self.metrics_url = f"{self.base_url}/metrics"
        self.health_url = f"{self.base_url}/health"
    
    async def check_metrics(self) -> Dict[str, Any]:
        """Fetch and parse metrics"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.metrics_url) as resp:
                    if resp.status == 200:
                        text = await resp.text()
                        return self.parse_metrics(text)
        except Exception as e:
            return {"error": str(e)}
        return {}
    
    def parse_metrics(self, text: str) -> Dict[str, Any]:
        """Parse Prometheus metrics"""
        metrics = {}
        for line in text.split('\n'):
            if line and not line.startswith('#'):
                if ' ' in line:
                    parts = line.split(' ')
                    try:
                        metrics[parts[0]] = float(parts[1])
                    except ValueError:
                        pass
        return metrics
    
    def check_health(self, metrics: Dict[str, Any]) -> tuple[bool, str]:
        """Check system health based on metrics"""
        checks = []
        
        # Latency check (P99 should be < 1ms)
        latency = metrics.get('hft_latency_p99_ns', 0)
        if latency > 1_000_000:
            checks.append(f"High latency: {latency/1000:.0f}μs")
        
        # Detection risk check (should be < 0.1%)
        detection = metrics.get('hft_detection_probability', 0)
        if detection > 0.001:
            checks.append(f"Detection risk: {detection*100:.3f}%")
        
        # Throughput check (should be > 10k ticks/sec)
        throughput = metrics.get('hft_ticks_per_second', 0)
        if throughput < 10000:
            checks.append(f"Low throughput: {throughput:.0f} ticks/sec")
        
        # Error rate check
        errors = metrics.get('hft_errors_total', 0)
        if errors > 100:
            checks.append(f"High error count: {errors:.0f}")
        
        is_healthy = len(checks) == 0
        message = "System healthy" if is_healthy else "; ".join(checks)
        
        return is_healthy, message
    
    async def run(self, verbose: bool = False) -> int:
        """Run health check"""
        metrics = await self.check_metrics()
        
        if "error" in metrics:
            print(f"ERROR: {metrics['error']}")
            return 1
        
        is_healthy, message = self.check_health(metrics)
        
        if verbose:
            print(json.dumps(metrics, indent=2))
        
        print(f"Health: {'✓' if is_healthy else '✗'} {message}")
        
        # Print key metrics
        print("\nKey Metrics:")
        print(f"  P99 Latency: {metrics.get('hft_latency_p99_ns', 0)/1000:.0f} μs")
        print(f"  Throughput: {metrics.get('hft_ticks_per_second', 0):.0f} ticks/s")
        print(f"  Detection Risk: {metrics.get('hft_detection_probability', 0)*100:.4f}%")
        print(f"  Total P&L: ${metrics.get('hft_total_pnl', 0):.2f}")
        print(f"  Sharpe Ratio: {metrics.get('hft_sharpe_ratio', 0):.3f}")
        
        return 0 if is_healthy else 1


def main():
    parser = argparse.ArgumentParser(description="HFT Health Check")
    parser.add_argument("--host", default="localhost", help="Host to check")
    parser.add_argument("--port", type=int, default=9090, help="Metrics port")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    
    args = parser.parse_args()
    
    checker = HealthChecker(args.host, args.port)
    exit_code = asyncio.run(checker.run(args.verbose))
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
