interface Transformer {
    int transform(int x);
}

class AnonInnerClass {
    public static void main(String[] args) {
        Transformer doubler = new Transformer() {
            public int transform(int x) { return x * 2; }
        };
        Transformer adder = new Transformer() {
            public int transform(int x) { return x + 10; }
        };
        System.out.println(doubler.transform(5));
        System.out.println(adder.transform(5));
    }
}
