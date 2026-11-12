//! The Configure text-field seam, not an INI reader. See configure.cpp:1413-1431:
//! empty/zero resets to 4; failed QString::toInt leaves the previous value intact.
//! settings.cpp:621-625 casts the signed int to native size_t; the initial batch
//! launcher uses min(size_t_limit, pending_count), with no upper clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcurrencyLimit(usize);

impl Default for ConcurrencyLimit {
    fn default() -> Self {
        Self(4)
    }
}

impl ConcurrencyLimit {
    pub fn new(value: usize) -> Result<Self, &'static str> {
        if value == 0 {
            Err("concurrency must be nonzero")
        } else {
            Ok(Self(value))
        }
    }

    pub fn get(self) -> usize {
        self.0
    }

    pub fn updated_from_configure_text(self, text: &str) -> Self {
        if text.is_empty() {
            return Self::default();
        }
        match text.trim().parse::<i32>() {
            Ok(0) => Self::default(),
            // Deliberate signed-int -> native size_t conversion, not a clamp.
            Ok(value) => Self((value as isize) as usize),
            Err(_) => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_limit_preserves_legacy_zero_empty_and_invalid_handling() {
        let previous = ConcurrencyLimit::new(7).expect("limit");
        for (text, expected) in [
            ("", 4),
            ("0", 4),
            (" 0 ", 4),
            ("bad", 7),
            ("   ", 7),
            ("2147483648", 7),
            ("100000", 100000),
        ] {
            assert_eq!(
                previous.updated_from_configure_text(text).get(),
                expected,
                "{text:?}"
            );
        }
        assert_eq!(previous.updated_from_configure_text("-1").get(), usize::MAX);
        assert_eq!(ConcurrencyLimit::default().get(), 4);
        assert!(ConcurrencyLimit::new(0).is_err());
        assert_eq!(
            ConcurrencyLimit::new(usize::MAX).expect("no cap").get(),
            usize::MAX
        );
    }
}
