pub fn choose(a: bool, b: bool) -> i32 {
    if a && b {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::choose;

    #[test]
    fn covers_both_choose_outcomes() {
        assert_eq!(choose(true, true), 1);
        assert_eq!(choose(false, true), 0);
    }
}
