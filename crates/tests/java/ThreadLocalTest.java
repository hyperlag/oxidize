class ThreadLocalTest {
    public static void main(String[] args) {
        ThreadLocal<Integer> threadLocal = ThreadLocal.withInitial(() -> {
            return 0;
        });

        // get default value
        int val = threadLocal.get();
        System.out.println("initial = " + val);

        // set
        threadLocal.set(42);
        System.out.println("after set = " + threadLocal.get());

        // remove resets to initial
        threadLocal.remove();
        System.out.println("after remove = " + threadLocal.get());

        System.out.println("done");
    }
}
