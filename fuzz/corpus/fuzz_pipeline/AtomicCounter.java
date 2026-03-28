import java.util.concurrent.atomic.AtomicInteger;

class AtomicCounter {
    public static void main(String[] args) {
        AtomicInteger counter = new AtomicInteger(5);
        counter.incrementAndGet();
        counter.addAndGet(1);
        System.out.println(counter.get());
        System.out.println(counter.getAndIncrement());
        System.out.println(counter.get());
        System.out.println(counter.compareAndSet(8, 10));
        System.out.println(counter.get());
    }
}
