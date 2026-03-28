class SynchronizedCounter {
    private int count = 0;

    public synchronized void increment() {
        count++;
    }

    public synchronized int getCount() {
        return count;
    }

    public static void main(String[] args) {
        SynchronizedCounter sc = new SynchronizedCounter();
        sc.increment();
        sc.increment();
        sc.increment();
        System.out.println(sc.getCount());
    }
}
