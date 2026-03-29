use tree_sitter::{Language, Node, Parser};

fn text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    std::str::from_utf8(&src[node.start_byte()..node.end_byte()]).unwrap_or("")
}

fn print_tree(node: Node, src: &[u8], depth: usize) {
    let indent = "  ".repeat(depth);
    let kind = node.kind();
    let is_named = node.is_named();
    if is_named {
        let _field = "";
        let txt = if node.child_count() == 0 {
            format!(" = {:?}", text(node, src))
        } else {
            String::new()
        };
        println!("{}{}{}", indent, kind, txt);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        print_tree(child, src, depth + 1);
    }
}

fn main() {
    let src = r#"
public class EnumTest {
    enum Color { RED, GREEN, BLUE }
    
    enum Planet {
        MERCURY(3.303e+23, 2.4397e6),
        EARTH(5.976e+24, 6.37814e6);
        
        private final double mass;
        private final double radius;
        
        Planet(double mass, double radius) {
            this.mass = mass;
            this.radius = radius;
        }
        
        double surfaceGravity() {
            double G = 6.67300E-11;
            return G * mass / (radius * radius);
        }
    }
}
"#;
    let mut parser = Parser::new();
    let language: Language = tree_sitter_java::language();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(src, None).unwrap();
    print_tree(tree.root_node(), src.as_bytes(), 0);
}
