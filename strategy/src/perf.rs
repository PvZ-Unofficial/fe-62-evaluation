//! Optional inclusive/exclusive function timing for optimization experiments.
#[cfg(not(feature = "perf-profile"))]
pub fn scope(_: &'static str) {}
#[cfg(not(feature = "perf-profile"))]
pub fn flush() {}
#[cfg(feature = "perf-profile")]
pub use measured::{flush, scope};
#[cfg(feature = "perf-profile")]
mod measured {
    use std::{cell::RefCell, collections::BTreeMap, time::Instant};

    #[derive(Default)]
    struct Timings {
        stack: Vec<(&'static str, u128)>,
        rows: BTreeMap<String, (u64, u128, u128)>,
    }
    thread_local! {
        static ENABLED: bool = std::env::var_os("SIX_PERF_DIR").is_some();
        static DATA: RefCell<Timings> = RefCell::new(Timings::default());
    }
    pub struct Scope(Option<Instant>);
    pub fn scope(name: &'static str) -> Scope {
        if !ENABLED.with(|x| *x) {
            return Scope(None);
        }
        DATA.with(|d| d.borrow_mut().stack.push((name, 0)));
        Scope(Some(Instant::now()))
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            let Some(start) = self.0 else {
                return;
            };
            let elapsed = start.elapsed().as_nanos();
            DATA.with(|d| {
                let mut d = d.borrow_mut();
                let (name, children) = d.stack.pop().unwrap();
                let mut path = d.stack.iter().map(|(n, _)| *n).collect::<Vec<_>>();
                path.push(name);
                let row = d.rows.entry(path.join(";")).or_default();
                row.0 += 1;
                row.1 += elapsed;
                row.2 += elapsed.saturating_sub(children);
                if let Some(parent) = d.stack.last_mut() {
                    parent.1 += elapsed;
                }
            });
        }
    }
    pub fn flush() {
        let Some(dir) = std::env::var_os("SIX_PERF_DIR") else {
            return;
        };
        DATA.with(|d| {
            let d = d.borrow();
            let mut text = String::from("stack\tcalls\tinclusive_ns\texclusive_ns\n");
            for (name, (calls, inc, exc)) in &d.rows {
                text.push_str(&format!("{name}\t{calls}\t{inc}\t{exc}\n"));
            }
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{:?}.tsv", std::thread::current().id())), text).unwrap();
        });
    }
}
