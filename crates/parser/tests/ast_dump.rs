//! Tree-sitter AST dump for generics exploration
//! Run: cargo test -p parser -- --nocapture dump_wildcard_ast

#[cfg(test)]
mod ast_dump {
    #[test]
    fn dump_wildcard_ast() {
        let src = r#"
import java.util.List;
class WildcardTest {
    public static void printList(List<?> list) {}
    public static double sum(List<? extends Number> numbers) { return 0; }
    public static void addNumbers(List<? super Integer> list) {}
}
class BoundedType<T extends Comparable<T>> {
    T value;
    public int compareTo(BoundedType<T> other) { return 0; }
}
class MultiBound<T extends Comparable<T> & Cloneable> {
    T value;
}
"#;
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_java::language();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(src, None).unwrap();
        print_node(tree.root_node(), src.as_bytes(), 0);
    }

    fn print_node(node: tree_sitter::Node, src: &[u8], indent: usize) {
        let text = std::str::from_utf8(&src[node.start_byte()..node.end_byte()]).unwrap_or("?");
        let short = if text.len() > 80 { &text[..80] } else { text };
        let short = short.replace('\n', "\\n");
        let pad = "  ".repeat(indent);
        let field = node.parent().and_then(|p| {
            (0..p.child_count()).find_map(|i| {
                if p.child(i)? == node {
                    p.field_name_for_child(i as u32)
                } else {
                    None
                }
            })
        });
        let field_str = field.map(|f| format!("{f}: ")).unwrap_or_default();
        if node.child_count() == 0 {
            println!("{pad}{field_str}{} = \"{short}\"", node.kind());
        } else {
            println!("{pad}{field_str}{}", node.kind());
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            print_node(child, src, indent + 1);
        }
    }
}
