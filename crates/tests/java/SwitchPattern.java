class Container {
    int value;
    String label;
    Container(int v, String l) {
        this.value = v;
        this.label = l;
    }
}

class SwitchPattern {
    static void describe(Container c) {
        // Pattern switch: type-check and bind in each arm.
        switch (c) {
            case Container x -> System.out.println(x.label + ": " + x.value);
            default -> System.out.println("unknown");
        }
    }

    public static void main(String[] args) {
        Container a = new Container(42, "alpha");
        Container b = new Container(99, "beta");
        describe(a);
        describe(b);
    }
}
