//! Accessor coverage shared by the deterministic mutation fuzzer and external
//! fuzzing engines. Never passes malformed input to the unchecked C++ runtime.
use crate::protoyaff::interop::Everything;

pub fn exercise(bytes: &[u8]) {
    if let Ok(m) = Everything::read(bytes) {
        let _ = m.get_id();
        let _ = m.has_zero();
        let _ = m.get_zero();
        let _ = m.get_text().and_then(|s| s.as_str());
        let _ = m.get_payload();
        let _ = m.get_state();
        let _ = m.which_choice();
        let _ = m.get_text_choice();
        let _ = m.find_lookup(b"a");
        let _ = m.find_labels(-2);
        let _ = m.get_flags().and_then(|a| a.get(0));
        let _ = m
            .get_numbers()
            .and_then(|a| a.get(a.len().saturating_sub(1)));
        let _ = m.get_words().and_then(|a| a.get_str(0));
        let _ = m.get_blobs().and_then(|a| a.get(0));
        let _ = m.get_states().and_then(|a| a.get(0));
        let _ = m
            .get_children()
            .and_then(|a| a.get(0))
            .and_then(|c| c.get_note());
        let _ = m
            .get_points()
            .and_then(|a| a.get(0))
            .and_then(|p| p.get_x());
        let _ = m.get_child().and_then(|c| c.get_counter());
        let _ = m.get_point().and_then(|p| p.get_y());
        let _ = m.get_sparse().and_then(|s| s.get_tiny_last());
        let _ = m.get_sparse().and_then(|s| s.get_wide_first());
        let _ = m.get_sparse().and_then(|s| s.get_last());
        let _ = m.get_defaults().and_then(|d| d.get_raw());
        let _ = m.get_defaults().and_then(|d| d.get_single());
        let _ = m.get_enabled();
        let _ = m.get_small();
        let _ = m.get_large();
        let _ = m.get_doubles().and_then(|a| a.get(0));
        let _ = m.get_timestamp().and_then(|t| t.get_nanos());
        if let Ok(mut node) = m.get_node() {
            for _ in 0..8 {
                let _ = node.get_value();
                if node.has_next() != Ok(true) {
                    break;
                }
                match node.get_next() {
                    Ok(next) => node = next,
                    Err(_) => break,
                }
            }
        }
    }
}
