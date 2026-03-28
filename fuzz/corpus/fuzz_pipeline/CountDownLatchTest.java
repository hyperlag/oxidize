import java.util.concurrent.CountDownLatch;

class CountDownLatchTest {
    public static void main(String[] args) {
        CountDownLatch latch = new CountDownLatch(3);
        System.out.println(latch.getCount());
        latch.countDown();
        latch.countDown();
        System.out.println(latch.getCount());
        latch.countDown();
        latch.await();
        System.out.println("latch done");
    }
}
