#!/usr/bin/env python3
# generate_market_data.py - Generate synthetic market data for testing

import struct
import random
import time
import math
import sys
from datetime import datetime, timedelta

# Constants
HEADER_SIZE = 64
TICK_SIZE = 64
MAGIC = b'HFTCAP01'
VERSION = 1

def generate_price_series(start_price, n_ticks, volatility, drift):
    """Generate geometric Brownian motion price series"""
    prices = [start_price]
    for _ in range(n_ticks - 1):
        ret = random.gauss(drift * 0.0001, volatility * 0.01)
        prices.append(prices[-1] * (1 + ret))
    return prices

def generate_volume_series(n_ticks, base_volume, volatility):
    """Generate volume series"""
    volumes = []
    for _ in range(n_ticks):
        vol = max(0.1, random.lognormvariate(math.log(base_volume), volatility))
        volumes.append(vol)
    return volumes

def write_market_data(filename, instruments, n_ticks_per_instrument, start_time):
    """Write market data to binary file"""
    
    total_ticks = n_ticks_per_instrument * len(instruments)
    
    with open(filename, 'wb') as f:
        # Write header
        header = struct.pack(
            '<8sIIQQQII24x',  # little-endian, 24 bytes reserved
            MAGIC,
            VERSION,
            len(instruments),
            total_ticks,
            int(start_time.timestamp() * 1e9),
            int((start_time + timedelta(seconds=n_ticks_per_instrument * 0.001)).timestamp() * 1e9),
            1000,  # sample_rate_hz
            0      # flags
        )
        f.write(header)
        
        # Write ticks for each instrument
        for inst_id, (symbol, start_price, volatility) in enumerate(instruments, 1):
            prices = generate_price_series(start_price, n_ticks_per_instrument, volatility, 0.0)
            volumes = generate_volume_series(n_ticks_per_instrument, 1000.0, 0.5)
            
            base_time = start_time
            for i in range(n_ticks_per_instrument):
                # Alternate between bid/ask/trade
                tick_type = i % 3
                side = 0 if tick_type in [0, 2] else 1
                
                # Bid/ask spread
                spread = 0.01 if tick_type == 0 else 0.0
                price = prices[i] + (spread if tick_type == 1 else 0)
                
                # Bid/ask depth
                bid_depth = 10000 + random.random() * 5000
                ask_depth = 10000 + random.random() * 5000
                
                tick = struct.pack(
                    '<ddQBBBIQdd8x',  # 8 bytes reserved at end
                    price,
                    volumes[i],
                    int(base_time.timestamp() * 1e9) + i * 1_000_000,  # 1ms intervals
                    1,  # exchange_id
                    side,
                    tick_type,
                    0,  # flags
                    i,  # sequence
                    inst_id,
                    i,  # trade_id
                    bid_depth,
                    ask_depth
                )
                f.write(tick)
    
    print(f"Generated {total_ticks} ticks for {len(instruments)} instruments")
    print(f"File size: {total_ticks * TICK_SIZE + HEADER_SIZE} bytes")

if __name__ == '__main__':
    instruments = [
        ("ES", 4500.0, 0.5),   # S&P 500
        ("CL", 75.0, 0.8),     # Crude Oil
        ("GC", 2000.0, 0.6),   # Gold
        ("EUR", 1.10, 0.3),    # Euro FX
    ]
    
    start_time = datetime.now() - timedelta(hours=1)
    write_market_data('market_data.bin', instruments, 100000, start_time)
