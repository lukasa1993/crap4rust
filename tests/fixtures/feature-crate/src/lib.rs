pub fn always(value: bool) -> bool {
    value
}

#[cfg(feature = "extra")]
pub fn feature_only(left: bool, right: bool) -> bool {
    if left && right {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_returns_input() {
        assert!(always(true));
        assert!(!always(false));
    }

    #[cfg(feature = "extra")]
    #[test]
    fn feature_only_covers_both_outcomes() {
        assert!(feature_only(true, true));
        assert!(!feature_only(true, false));
    }
}
