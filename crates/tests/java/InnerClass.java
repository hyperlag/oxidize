class InnerClass {
    class Counter {
        private int count = 0;

        void inc() { count = count + 1; }
        int get() { return count; }
    }

    void run() {
        Counter c = new Counter();
        c.inc();
        c.inc();
        c.inc();
        System.out.println(c.get());
    }

    public static void main(String[] args) {
        InnerClass obj = new InnerClass();
        obj.run();
    }
}
