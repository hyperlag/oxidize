// Stage 22: pattern switch expression — switch used as a value with
// type-pattern arms (`case Type binding ->`).
class Shape {
    String kind;
    int size;
    Shape(String k, int s) { this.kind = k; this.size = s; }
}

class PatternSwitchExpr {
    static String describe(Shape s) {
        return switch (s) {
            case Shape x -> x.kind + ":" + x.size;
            default -> "other";
        };
    }

    public static void main(String[] args) {
        System.out.println(describe(new Shape("circle", 5)));
        System.out.println(describe(new Shape("square", 3)));
    }
}
