public class InterfaceDefault {
    interface Greeter {
        String name();

        // default method — provided implementation
        default String greet() {
            return "Hello, " + name() + "!";
        }

        // second default method
        default String farewell() {
            return "Goodbye, " + name() + ".";
        }
    }

    // Inherits greet() unchanged, overrides farewell()
    static class FormalGreeter implements Greeter {
        private String n;
        FormalGreeter(String n) { this.n = n; }

        @Override
        public String name() { return n; }

        @Override
        public String farewell() {
            return "Farewell, " + name() + ".";
        }
    }

    // Inherits both default methods without overriding
    static class CasualGreeter implements Greeter {
        private String n;
        CasualGreeter(String n) { this.n = n; }

        @Override
        public String name() { return n; }
    }

    public static void main(String[] args) {
        FormalGreeter f = new FormalGreeter("Alice");
        System.out.println(f.greet());     // Hello, Alice!  (default)
        System.out.println(f.farewell());  // Farewell, Alice.  (overridden)

        CasualGreeter c = new CasualGreeter("Bob");
        System.out.println(c.greet());     // Hello, Bob!    (default)
        System.out.println(c.farewell());  // Goodbye, Bob.  (default)
    }
}
