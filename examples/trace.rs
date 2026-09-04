//! Debug harness: simulates Clippy's behaviour and prints statistics.
//! Not part of the app. Run with `cargo run --example trace`.

use clippy_app::agent::Agent;
use clippy_app::companion::Companion;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

fn is_idle(name: &str) -> bool {
    name.starts_with("Idle")
}

fn main() {
    // Long session with periodic pokes (seed 6).
    {
        let t0 = Instant::now();
        let mut companion = Companion::with_seed(Agent::clippy(), t0, 6);
        let mut now = t0;
        let end = t0 + Duration::from_secs(45 * 60);
        let mut seen: Vec<String> = Vec::new();
        let mut idles: Vec<String> = Vec::new();
        let mut next_poke = t0 + Duration::from_secs(7);
        let mut steps = 0u64;
        while now < end {
            steps += 1;
            let name = companion.animation().unwrap().to_string();
            if seen.last() != Some(&name) {
                if is_idle(&name) {
                    idles.push(name.clone());
                }
                seen.push(name);
            }
            let deadline = companion.next_deadline().unwrap();
            if next_poke < deadline {
                now = now.max(next_poke);
                companion.poke(now);
                next_poke = now + Duration::from_secs(7);
            } else {
                now = now.max(deadline);
                companion.advance(now);
            }
        }
        let distinct: BTreeSet<&String> = seen.iter().collect();
        println!(
            "[session] steps={steps} transitions={} distinct={} idles={}",
            seen.len(),
            distinct.len(),
            idles.len()
        );
        println!("[session] distinct names: {distinct:?}");
        println!(
            "[session] first 30 transitions: {:?}",
            &seen[..seen.len().min(30)]
        );
    }

    // Repeated pokes while resting (seed 5).
    {
        let t0 = Instant::now();
        let mut companion = Companion::with_seed(Agent::clippy(), t0, 5);
        let mut now = t0;
        // rest first
        let mut guard = 0;
        while !companion.is_resting() {
            now = now.max(companion.next_deadline().unwrap());
            companion.advance(now);
            guard += 1;
            if guard > 10_000 {
                break;
            }
        }
        println!(
            "[pokes] resting after {guard} steps at animation {:?}",
            companion.animation()
        );
        companion.poke(now);
        let first = companion.animation().unwrap().to_string();
        companion.poke(now + Duration::from_millis(10));
        companion.poke(now + Duration::from_millis(20));
        companion.poke(now + Duration::from_millis(30));
        println!("[pokes] first gesture: {first}");
        let mut names: Vec<String> = Vec::new();
        let mut last: Option<String> = None;
        let mut clock = now;
        for _ in 0..20_000 {
            if companion.is_resting() && !names.is_empty() {
                break;
            }
            let name = companion.animation().unwrap().to_string();
            if last.as_deref() != Some(name.as_str()) {
                names.push(name.clone());
                last = Some(name);
            }
            clock = clock.max(companion.next_deadline().unwrap());
            companion.advance(clock);
        }
        println!("[pokes] names played: {names:?} (len {})", names.len());
    }
}
