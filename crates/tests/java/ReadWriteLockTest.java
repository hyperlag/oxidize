import java.util.concurrent.locks.ReentrantReadWriteLock;

class ReadWriteLockTest {
    public static void main(String[] args) {
        ReentrantReadWriteLock rwLock = new ReentrantReadWriteLock();
        int sharedData = 0;

        // Write lock
        rwLock.writeLock().lock();
        sharedData = 100;
        rwLock.writeLock().unlock();
        System.out.println("after write: " + sharedData);

        // Read lock
        rwLock.readLock().lock();
        int val = sharedData;
        rwLock.readLock().unlock();
        System.out.println("after read: " + val);

        // tryLock on read
        boolean gotRead = rwLock.readLock().tryLock();
        System.out.println("tryLock read = " + gotRead);
        if (gotRead) {
            rwLock.readLock().unlock();
        }

        // tryLock on write
        boolean gotWrite = rwLock.writeLock().tryLock();
        System.out.println("tryLock write = " + gotWrite);
        if (gotWrite) {
            rwLock.writeLock().unlock();
        }

        System.out.println("done");
    }
}
