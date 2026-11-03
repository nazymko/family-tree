#![no_main]
use gt_core::date::{DayOrder, GenDate};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    for order in [DayOrder::Dmy, DayOrder::Mdy] {
        let d = GenDate::parse(&s, order);
        let _ = d.display();
    }
});
