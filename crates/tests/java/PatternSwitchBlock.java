// Stage 22 extra: pattern switch expression with block/yield arm.
class Widget {
    String label;
    int count;
    Widget(String l, int c) { this.label = l; this.count = c; }
}

class PatternSwitchBlock {
    // Block arm: uses { ... yield ...; } syntax.
    static String describe(Widget w) {
        return switch (w) {
            case Widget x -> {
                int doubled = x.count * 2;
                yield x.label + ":" + doubled;
            }
            default -> "other";
        };
    }

    public static void main(String[] args) {
        System.out.println(describe(new Widget("widget", 3)));
        System.out.println(describe(new Widget("gadget", 5)));
    }
}
