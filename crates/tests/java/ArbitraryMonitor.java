// Test per-object synchronized monitors.
// Nesting synchronized blocks on two DIFFERENT objects proves that each object
// has its own independent monitor: with a single global lock the second lock
// attempt would deadlock, but with per-object monitors it succeeds.
class Resource {
    int value = 0;
}

class ArbitraryMonitor {
    public static void main(String[] args) {
        Resource r1 = new Resource();
        Resource r2 = new Resource();

        synchronized (r1) {
            r1.value = 1;
            synchronized (r2) {
                r2.value = 2;
            }
        }

        System.out.println(r1.value);
        System.out.println(r2.value);
    }
}
