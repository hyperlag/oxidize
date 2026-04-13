import java.util.concurrent.locks.StampedLock;

class StampedLockTest {
    static int value = 0;
    static StampedLock sl = new StampedLock();

    // Write value protected by write lock (no try-finally to avoid codegen limitation)
    static void writeValue(int v) {
        long stamp = sl.writeLock();
        value = v;
        sl.unlockWrite(stamp);
    }

    // Read value using pessimistic read lock
    static int readValue() {
        long stamp = sl.readLock();
        int result = value;
        sl.unlockRead(stamp);
        return result;
    }

    // Read value using optimistic read
    static int optimisticRead() {
        long stamp = sl.tryOptimisticRead();
        int v = value;
        if (!sl.validate(stamp)) {
            stamp = sl.readLock();
            v = value;
            sl.unlockRead(stamp);
        }
        return v;
    }

    public static void main(String[] args) {
        writeValue(42);
        System.out.println(readValue());
        System.out.println(optimisticRead());
        writeValue(100);
        System.out.println(readValue());
    }
}
