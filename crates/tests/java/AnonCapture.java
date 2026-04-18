interface Greeter {
    String greet();
}

interface Adder {
    int add(int x);
}

class AnonCapture {
    public static void main(String[] args) {
        String name = "World";
        int base = 100;

        Greeter g = new Greeter() {
            public String greet() {
                return "Hello, " + name + "!";
            }
        };

        Adder a = new Adder() {
            public int add(int x) {
                return base + x;
            }
        };

        System.out.println(g.greet());
        System.out.println(a.add(42));
    }
}
