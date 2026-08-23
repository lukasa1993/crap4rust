use std::fs;

fn main() {
    let path = "src/lib.rs";
    let mut source = fs::read_to_string(path).unwrap();
    let old = r###"    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.value += 1;
        if node.guard.is_some() {
            self.value += 1;
        }
        visit::visit_arm(self, node);
    }"###;
    let new = r###"    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.value += 1;
        visit::visit_arm(self, node);
    }

    fn visit_pat_guard(&mut self, node: &'ast syn::PatGuard) {
        self.value += 1;
        visit::visit_pat_guard(self, node);
    }"###;
    let start = source.find(old).expect("missing Syn 3 guard patch anchor");
    assert!(
        source[start + old.len()..].find(old).is_none(),
        "duplicate Syn 3 guard patch anchor"
    );
    source.replace_range(start..start + old.len(), new);
    fs::write(path, source).unwrap();
}
