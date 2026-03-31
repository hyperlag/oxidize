import java.util.concurrent.locks.ReentrantLock;
import java.util.concurrent.locks.Condition;

class ReentrantLockTest {
    public static void main(String[] args) {
        ReentrantLock lock = new ReentrantLock();

        // Basic lock/unlock
        lock.lock();
        int counter = 42;
        lock.unlock();
        System.out.println("counter = " + counter);

        // tryLock
        boolean acquired = lock.tryLock();
        System.out.println("tryLock = " + acquired);
        if (acquired) {
            lock.unlock();
        }

        // Condition
        Condition cond = lock.newCondition();
        System.out.println("condition created");

        System.out.println("done");
    }
}
