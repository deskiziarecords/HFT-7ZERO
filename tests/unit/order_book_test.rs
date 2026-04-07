// ============================================================
// ORDER BOOK UNIT TESTS
// ============================================================
// Basic order book operations
// Price level management
// Depth calculations
// Edge cases and concurrency
// ============================================================

use hft_stealth_system::market::order_book::*;
use hft_stealth_system::market::tick::*;

// ============================================================
// BASIC OPERATIONS TESTS
// ============================================================

#[test]
fn test_new_order_book() {
    let book = OrderBook::new(1, 0.01);
    assert_eq!(book.best_bid(), 0.0);
    assert_eq!(book.best_ask(), f64::INFINITY);
    assert_eq!(book.spread(), f64::INFINITY);
}

#[test]
fn test_add_bid() {
    let mut book = OrderBook::new(1, 0.01);
    let bid = Tick::bid(100.00, 1000.0, 1000, 1);
    
    book.update(&bid);
    
    assert_eq!(book.best_bid(), 100.00);
    assert_eq!(book.bid_at_depth(0).unwrap().volume, 1000.0);
}

#[test]
fn test_add_ask() {
    let mut book = OrderBook::new(1, 0.01);
    let ask = Tick::ask(100.05, 1000.0, 1000, 1);
    
    book.update(&ask);
    
    assert_eq!(book.best_ask(), 100.05);
    assert_eq!(book.ask_at_depth(0).unwrap().volume, 1000.0);
}

#[test]
fn test_best_bid_ask() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Add multiple bids
    book.update(&Tick::bid(100.00, 1000.0, 1000, 1));
    book.update(&Tick::bid(100.01, 500.0, 1001, 1));
    book.update(&Tick::bid(99.99, 2000.0, 1002, 1));
    
    // Best bid should be highest price
    assert_eq!(book.best_bid(), 100.01);
    
    // Add multiple asks
    book.update(&Tick::ask(100.05, 1000.0, 1003, 1));
    book.update(&Tick::ask(100.04, 500.0, 1004, 1));
    book.update(&Tick::ask(100.06, 2000.0, 1005, 1));
    
    // Best ask should be lowest price
    assert_eq!(book.best_ask(), 100.04);
}

#[test]
fn test_remove_bid() {
    let mut book = OrderBook::new(1, 0.01);
    
    book.update(&Tick::bid(100.00, 1000.0, 1000, 1));
    assert_eq!(book.best_bid(), 100.00);
    
    // Remove with zero volume
    book.update(&Tick::bid(100.00, 0.0, 1001, 1));
    
    // Best bid should be gone
    assert_eq!(book.best_bid(), 0.0);
}

#[test]
fn test_remove_ask() {
    let mut book = OrderBook::new(1, 0.01);
    
    book.update(&Tick::ask(100.05, 1000.0, 1000, 1));
    assert_eq!(book.best_ask(), 100.05);
    
    // Remove with zero volume
    book.update(&Tick::ask(100.05, 0.0, 1001, 1));
    
    // Best ask should be gone
    assert_eq!(book.best_ask(), f64::INFINITY);
}

// ============================================================
// DEPTH TESTS
// ============================================================

#[test]
fn test_depth_levels() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Add depth
    for i in 0..10 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0 * (10 - i) as f64, 1000 + i as u64, 1);
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0 * (10 - i) as f64, 2000 + i as u64, 1);
        book.update(&bid);
        book.update(&ask);
    }
    
    // Check bid depth
    for i in 0..5 {
        let level = book.bid_at_depth(i).unwrap();
        let expected_price = 100.00 - i as f64 * 0.01;
        assert!((level.price - expected_price).abs() < 0.001);
    }
    
    // Check ask depth
    for i in 0..5 {
        let level = book.ask_at_depth(i).unwrap();
        let expected_price = 100.05 + i as f64 * 0.01;
        assert!((level.price - expected_price).abs() < 0.001);
    }
}

#[test]
fn test_total_volume() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Add bids
    for i in 0..5 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0, 1000 + i as u64, 1);
        book.update(&bid);
    }
    
    let total_bid = book.total_bid_volume();
    assert_eq!(total_bid, 5000.0);
    
    // Add asks
    for i in 0..3 {
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0, 2000 + i as u64, 1);
        book.update(&ask);
    }
    
    let total_ask = book.total_ask_volume();
    assert_eq!(total_ask, 3000.0);
}

// ============================================================
// ORDER IMBALANCE TESTS
// ============================================================

#[test]
fn test_order_imbalance() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Heavy bid side
    for i in 0..10 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0, 1000 + i as u64, 1);
        book.update(&bid);
    }
    
    // Light ask side
    for i in 0..3 {
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 100.0, 2000 + i as u64, 1);
        book.update(&ask);
    }
    
    let imbalance = book.order_imbalance();
    assert!(imbalance > 0.7, "Imbalance should be positive: {}", imbalance);
}

#[test]
fn test_balanced_book() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Equal bid and ask depth
    for i in 0..5 {
        let bid = Tick::bid(100.00 - i as f64 * 0.01, 1000.0, 1000 + i as u64, 1);
        let ask = Tick::ask(100.05 + i as f64 * 0.01, 1000.0, 2000 + i as u64, 1);
        book.update(&bid);
        book.update(&ask);
    }
    
    let imbalance = book.order_imbalance();
    assert!((imbalance).abs() < 0.1, "Imbalance should be near zero: {}", imbalance);
}

// ============================================================
// SPREAD TESTS
// ============================================================

#[test]
fn test_spread_calculation() {
    let mut book = OrderBook::new(1, 0.01);
    
    book.update(&Tick::bid(100.00, 1000.0, 1000, 1));
    book.update(&Tick::ask(100.05, 1000.0, 1001, 1));
    
    assert_eq!(book.spread(), 0.05);
    assert_eq!(book.spread_ticks(), 5);
}

#[test]
fn test_widening_spread() {
    let mut book = OrderBook::new(1, 0.01);
    
    book.update(&Tick::bid(100.00, 1000.0, 1000, 1));
    book.update(&Tick::ask(100.10, 1000.0, 1001, 1));
    
    assert_eq!(book.spread(), 0.10);
    assert_eq!(book.spread_ticks(), 10);
}

// ============================================================
// MID PRICE TESTS
// ============================================================

#[test]
fn test_mid_price() {
    let mut book = OrderBook::new(1, 0.01);
    
    book.update(&Tick::bid(100.00, 1000.0, 1000, 1));
    book.update(&Tick::ask(100.05, 1000.0, 1001, 1));
    
    assert_eq!(book.mid_price(), 100.025);
}

#[test]
fn test_weighted_mid() {
    let mut book = OrderBook::new(1, 0.01);
    
    // Heavy bid volume
    book.update(&Tick::bid(100.00, 10000.0, 1000, 1));
    book.update(&Tick::ask(100.05, 1000.0, 1001, 1));
    
    let weighted_mid = book.weighted_mid();
    // Should be closer to bid due to higher volume
    assert!(weighted_mid < book.mid_price());
}

// ============================================================
// TOP LEVEL
